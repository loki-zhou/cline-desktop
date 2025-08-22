use tonic::transport::Channel;
use tonic::Request;
use serde_json::Value;

use crate::grpc_client::{
    cline::{mcp_service_client::McpServiceClient, EmptyRequest, Metadata, Empty},
    types::{GrpcResult, StreamConfig},
    utils::{with_timeout, log_debug, log_success, DEFAULT_REQUEST_TIMEOUT},
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
        if let Some(client) = &mut self.client {
            log_debug("Calling subscribeToMcpServers on cline-core");
            
            let request = Request::new(EmptyRequest {
                metadata: Some(Metadata {}),
            });
            
            let mut stream = with_timeout(
                client.subscribe_to_mcp_servers(request),
                DEFAULT_REQUEST_TIMEOUT,
                "subscribeToMcpServers"
            ).await?.into_inner();
            
            // 如果配置了流式处理，则持续监听
            if let Some(config) = stream_config {
                if config.enable_streaming {
                    return self.handle_streaming_mcp_servers(stream, config).await;
                }
            }
            
            // 默认行为：只返回第一个响应
            if let Some(servers_result) = stream.message().await? {
                log_success(&format!("Received MCP servers from subscribeToMcpServers, count: {}", 
                    servers_result.mcp_servers.len()));
                
                let servers_value = self.build_mcp_servers_response(&servers_result);
                return Ok(servers_value);
            }
            
            Err("No MCP servers received from stream".into())
        } else {
            Err("No McpService gRPC client available".into())
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
}

impl McpServiceHandler {
    pub async fn handle_request(&mut self, method: &str, _message: &Value) -> GrpcResult<Value> {
        self.handle_request_with_config(method, _message, None).await
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