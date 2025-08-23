use tonic::transport::Channel;
use tonic::Request;
use serde_json::Value;

use crate::grpc_client::{
    cline::{mcp_service_client::McpServiceClient, EmptyRequest, Metadata, Empty},
    types::{GrpcResult, StreamConfig},
    utils::{with_timeout, log_debug, log_success, log_error, DEFAULT_REQUEST_TIMEOUT},
};

#[derive(Debug)]
pub struct McpServiceHandler {
    client: Option<McpServiceClient<Channel>>,
}

impl McpServiceHandler {
    pub fn new() -> Self {
        Self { client: None }
    }
    
    pub fn set_client(&mut self, channel: Channel) {
        self.client = Some(McpServiceClient::new(channel));
    }
    
    async fn get_latest_mcp_servers(&mut self) -> GrpcResult<Value> {
        if let Some(client) = &mut self.client {
            log_debug("Calling getLatestMcpServers on cline-core");
            
            let request = Request::new(Empty {});
            
            let response = with_timeout(
                client.get_latest_mcp_servers(request),
                DEFAULT_REQUEST_TIMEOUT,
                "getLatestMcpServers"
            ).await?;
            
            let mcp_servers = response.into_inner();
            log_success(&format!("Received MCP servers from cline-core, count: {}", 
                mcp_servers.mcp_servers.len()));
            
            // 手动构造响应
            let servers_value = self.build_mcp_servers_response(&mcp_servers);
            Ok(servers_value)
        } else {
            Err("No McpService gRPC client available".into())
        }
    }
    
    async fn subscribe_to_mcp_servers(&mut self) -> GrpcResult<Value> {
        self.subscribe_to_mcp_servers_with_config(None).await
    }
    
    async fn subscribe_to_mcp_servers_with_config(&mut self, stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        log_debug("[McpService] Starting subscribeToMcpServers with new logic");
        
        if let Some(client) = &mut self.client {
            log_debug("Calling subscribeToMcpServers on cline-core");
            
            let request = Request::new(EmptyRequest {
                metadata: Some(Metadata {}),
            });
            
            // 建立流式连接，但不等待第一个消息
            match client.subscribe_to_mcp_servers(request).await {
                Ok(stream_result) => {
                    let stream = stream_result.into_inner();
                    
                    log_success("[McpService] Successfully established MCP servers subscription - returning immediately");
                    
                    // 根据 cline 原始逻辑，MCP 订阅应该始终保持活跃
                    // 即使没有显式的流式配置，我们也在后台处理推送
                    if let Some(config) = stream_config {
                        if config.enable_streaming {
                            log_debug("[McpService] Starting background stream processing with config");
                            tokio::spawn(async move {
                                let _ = Self::handle_background_mcp_servers_stream(stream, config).await;
                            });
                        } else {
                            log_debug("[McpService] Starting default background stream processing");
                            // 即使没有启用显式流式处理，也要保持连接以接收 McpHub 的状态推送
                            tokio::spawn(async move {
                                let _ = Self::handle_default_mcp_servers_stream(stream).await;
                            });
                        }
                    } else {
                        log_debug("[McpService] Starting default background stream processing (no config)");
                        // 没有配置时，使用默认的流式处理来接收 McpHub 推送
                        tokio::spawn(async move {
                            let _ = Self::handle_default_mcp_servers_stream(stream).await;
                        });
                    }
                    
                    // 立即返回订阅成功状态
                    let success_response = serde_json::json!({
                        "subscription_established": true,
                        "message": "Successfully subscribed to MCP servers updates",
                        "type": "subscription",
                        "service": "McpService",
                        "method": "subscribeToMcpServers"
                    });
                    
                    log_success(&format!("[McpService] Returning success response: {}", success_response));
                    return Ok(success_response);
                }
                Err(e) => {
                    let error_msg = format!("Failed to establish MCP servers subscription: {}", e);
                    log_error(&format!("[McpService] {}", error_msg));
                    return Err(error_msg.into());
                }
            }
        } else {
            let error_msg = "No McpService gRPC client available";
            log_error(&format!("[McpService] {}", error_msg));
            Err(error_msg.into())
        }
    }
    
    async fn handle_streaming_mcp_servers(
        &mut self, 
        mut stream: tonic::Streaming<crate::grpc_client::cline::McpServers>,
        config: StreamConfig
    ) -> GrpcResult<Value> {
        log_debug("Starting streaming MCP servers processing");
        
        let mut message_count = 0;
        let max_messages = config.max_messages.unwrap_or(usize::MAX);
        let mut last_servers: Option<Value> = None;
        
        while let Some(servers_result) = stream.message().await? {
            let servers_value = self.build_mcp_servers_response(&servers_result);
            
            // 如果有回调，调用它
            if let Some(ref callback) = config.callback {
                if let Err(e) = callback(servers_value.clone()) {
                    log_debug(&format!("Stream callback error: {}", e));
                }
            }
            
            last_servers = Some(servers_value);
            message_count += 1;
            
            log_debug(&format!("Processed streaming MCP servers message {}/{}", message_count, max_messages));
            
            // 检查是否达到最大消息数量
            if message_count >= max_messages {
                log_debug("Reached maximum message limit, stopping stream");
                break;
            }
        }
        
        log_success(&format!("Streaming MCP servers processing completed, processed {} messages", message_count));
        
        // 返回最后一条消息或默认值
        Ok(last_servers.unwrap_or_else(|| serde_json::json!({
            "streaming": true,
            "messages_processed": message_count
        })))
    }
    
    // 辅助方法：构造 MCP 服务器响应
    fn build_mcp_servers_response(&self, mcp_servers: &crate::grpc_client::cline::McpServers) -> Value {
        serde_json::json!({
            "mcp_servers": mcp_servers.mcp_servers.iter().map(|server| {
                serde_json::json!({
                    "name": server.name,
                    "config": server.config,
                    "status": server.status as i32,
                    "error": server.error,
                    "disabled": server.disabled,
                    "timeout": server.timeout,
                    "tools": server.tools.iter().map(|tool| {
                        serde_json::json!({
                            "name": tool.name,
                            "description": tool.description,
                            "input_schema": tool.input_schema,
                            "auto_approve": tool.auto_approve
                        })
                    }).collect::<Vec<_>>(),
                    "resources": server.resources.iter().map(|resource| {
                        serde_json::json!({
                            "uri": resource.uri,
                            "name": resource.name,
                            "mime_type": resource.mime_type,
                            "description": resource.description
                        })
                    }).collect::<Vec<_>>(),
                    "resource_templates": server.resource_templates.iter().map(|template| {
                        serde_json::json!({
                            "uri_template": template.uri_template,
                            "name": template.name,
                            "mime_type": template.mime_type,
                            "description": template.description
                        })
                    }).collect::<Vec<_>>()
                })
            }).collect::<Vec<_>>()
        })
    }
    
    // 静态方法：构造 MCP 服务器响应（用于后台处理）
    fn build_static_mcp_servers_response(mcp_servers: &crate::grpc_client::cline::McpServers) -> Value {
        serde_json::json!({
            "mcp_servers": mcp_servers.mcp_servers.iter().map(|server| {
                serde_json::json!({
                    "name": server.name,
                    "config": server.config,
                    "status": server.status as i32,
                    "error": server.error,
                    "disabled": server.disabled,
                    "timeout": server.timeout,
                    "tools": server.tools.iter().map(|tool| {
                        serde_json::json!({
                            "name": tool.name,
                            "description": tool.description,
                            "input_schema": tool.input_schema,
                            "auto_approve": tool.auto_approve
                        })
                    }).collect::<Vec<_>>(),
                    "resources": server.resources.iter().map(|resource| {
                        serde_json::json!({
                            "uri": resource.uri,
                            "name": resource.name,
                            "mime_type": resource.mime_type,
                            "description": resource.description
                        })
                    }).collect::<Vec<_>>(),
                    "resource_templates": server.resource_templates.iter().map(|template| {
                        serde_json::json!({
                            "uri_template": template.uri_template,
                            "name": template.name,
                            "mime_type": template.mime_type,
                            "description": template.description
                        })
                    }).collect::<Vec<_>>()
                })
            }).collect::<Vec<_>>()
        })
    }
    
    // 静态方法：在后台处理 MCP 服务器流式数据
    async fn handle_background_mcp_servers_stream(
        mut stream: tonic::Streaming<crate::grpc_client::cline::McpServers>,
        config: StreamConfig
    ) -> GrpcResult<()> {
        log_debug("Starting background MCP servers stream processing");
        
        let mut message_count = 0;
        let max_messages = config.max_messages.unwrap_or(usize::MAX);
        
        while let Some(servers_result) = stream.message().await.map_err(|e| {
            format!("Stream error: {}", e)
        })? {
            let servers_value = Self::build_static_mcp_servers_response(&servers_result);
            
            // 如果有回调，调用它
            if let Some(ref callback) = config.callback {
                if let Err(e) = callback(servers_value) {
                    log_debug(&format!("Background stream callback error: {}", e));
                }
            }
            
            message_count += 1;
            log_debug(&format!("Processed background MCP servers message {}/{}", message_count, max_messages));
            
            // 检查是否达到最大消息数量
            if message_count >= max_messages {
                log_debug("Reached maximum message limit in background stream");
                break;
            }
        }
        
        log_success(&format!("Background MCP servers stream processing completed, processed {} messages", message_count));
        Ok(())
    }
    
    // 静态方法：处理默认的 MCP 服务器流式数据（根据 cline 原始逻辑）
    async fn handle_default_mcp_servers_stream(
        mut stream: tonic::Streaming<crate::grpc_client::cline::McpServers>
    ) -> GrpcResult<()> {
        log_debug("[McpService] Starting default MCP servers stream processing - maintaining active connection for McpHub updates");
        
        let mut message_count = 0;
        
        // 根据 cline 原始逻辑，保持流连接活跃以接收 McpHub 的实时推送
        while let Some(servers_result) = stream.message().await.map_err(|e| {
            log_error(&format!("[McpService] Default stream error: {}", e));
            format!("Default stream error: {}", e)
        })? {
            message_count += 1;
            
            // 构建服务器状态响应
            let servers_value = Self::build_static_mcp_servers_response(&servers_result);
            
            log_debug(&format!(
                "[McpService] Received McpHub status update #{}: {} servers", 
                message_count,
                servers_result.mcp_servers.len()
            ));
            
            // 在这里可以将状态更新转发给前端
            // 这模拟了原始 cline 中 McpHub.notifyWebviewOfServerChanges() 的功能
            log_success(&format!(
                "[McpService] Processed MCP server state update: {:?}",
                servers_result.mcp_servers.iter().map(|s| format!("{}({})", s.name, s.status)).collect::<Vec<_>>()
            ));
        }
        
        log_success(&format!(
            "[McpService] Default MCP servers stream completed, processed {} updates from McpHub", 
            message_count
        ));
        Ok(())
    }
}

impl McpServiceHandler {
    pub async fn handle_request(&mut self, method: &str, message: &Value) -> GrpcResult<Value> {
        // 对于订阅方法，传递空的配置以使用被动订阅模式
        if matches!(method, "subscribeToMcpServers") {
            // 对于订阅服务，不启用流式处理，立即返回成功
            self.handle_request_with_config(method, message, None).await
        } else {
            self.handle_request_with_config(method, message, None).await
        }
    }
    
    pub async fn handle_request_with_config(&mut self, method: &str, _message: &Value, stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        match method {
            "getLatestMcpServers" => self.get_latest_mcp_servers().await,
            "subscribeToMcpServers" => self.subscribe_to_mcp_servers_with_config(stream_config).await,
            _ => {
                log_debug(&format!("McpService method not implemented: {}", method));
                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("McpService method {} not implemented yet", method)
                }))
            }
        }
    }
}