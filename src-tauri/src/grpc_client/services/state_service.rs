use tonic::transport::Channel;
use tonic::Request;
use serde_json::Value;

use crate::grpc_client::{
    cline::{state_service_client::StateServiceClient, EmptyRequest, Metadata},
    types::{GrpcResult, StreamConfig},
    utils::{with_timeout, log_debug, log_success, DEFAULT_REQUEST_TIMEOUT},
};

#[derive(Debug)]
pub struct StateServiceHandler {
    client: Option<StateServiceClient<Channel>>,
}

impl StateServiceHandler {
    pub fn new() -> Self {
        Self { client: None }
    }
    
    pub fn set_client(&mut self, channel: Channel) {
        self.client = Some(StateServiceClient::new(channel));
    }
    
    async fn get_latest_state(&mut self) -> GrpcResult<Value> {
        if let Some(client) = &mut self.client {
            log_debug("Calling getLatestState on cline-core");
            
            let request = Request::new(EmptyRequest {
                metadata: Some(Metadata {}),
            });
            
            let response = with_timeout(
                client.get_latest_state(request),
                DEFAULT_REQUEST_TIMEOUT,
                "getLatestState"
            ).await?;
            
            let state = response.into_inner();
            log_success(&format!("Received state from cline-core, state_json length: {}", 
                state.state_json.len()));
            
            // 返回正确的 State 消息结构，保持 stateJson 字段
            let state_response = serde_json::json!({
                "stateJson": state.state_json
            });
            
            println!("[DEBUG] ===== RETURNING STATE RESPONSE TO FRONTEND =====");
            println!("[DEBUG] State response structure: {}", 
                serde_json::to_string_pretty(&state_response).unwrap_or_else(|_| "Invalid JSON".to_string()));
            
            Ok(state_response)
        } else {
            Err("No StateService gRPC client available".into())
        }
    }
    
    async fn subscribe_to_state(&mut self) -> GrpcResult<Value> {
        self.subscribe_to_state_with_config(None).await
    }
    
    async fn subscribe_to_state_with_config(&mut self, stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        println!("[DEBUG] ===== StateService.subscribe_to_state_with_config CALLED =====");
        println!("[DEBUG] stream_config: {:?}", stream_config);
        
        if let Some(client) = &mut self.client {
            println!("[DEBUG] StateService client is available");
            log_debug("Calling subscribeToState on cline-core");
            
            println!("[DEBUG] Creating gRPC request for subscribeToState");
            let request = Request::new(EmptyRequest {
                metadata: Some(Metadata {}),
            });
            
            println!("[DEBUG] Sending gRPC request to cline-core on port 26040");
            match client.subscribe_to_state(request).await {
                Ok(stream_result) => {
                    let mut stream = stream_result.into_inner();
                    println!("[DEBUG] Successfully got stream from cline-core, waiting for first state...");
                    
                    // 等待第一个状态消息（这是前端需要的初始状态）
                    if let Some(state_result) = stream.message().await? {
                        println!("[DEBUG] ===== RECEIVED INITIAL STATE FROM CLINE-CORE =====");
                        log_success(&format!("Received initial state from subscribeToState, state_json length: {}", 
                            state_result.state_json.len()));
                        println!("[DEBUG] Raw state_json (first 200 chars): {}", 
                            if state_result.state_json.len() > 200 { 
                                &state_result.state_json[..200] 
                            } else { 
                                &state_result.state_json 
                            });
                        
                        // 在后台继续处理流以接收后续状态更新
                        println!("[DEBUG] Starting background stream processing for subsequent updates");
                        tokio::spawn(async move {
                            let _ = Self::handle_default_state_stream(stream).await;
                        });
                        
                        // 返回初始状态给前端
                        let state_response = serde_json::json!({
                            "stateJson": state_result.state_json
                        });
                        
                        println!("[DEBUG] ===== RETURNING INITIAL STATE RESPONSE TO FRONTEND =====");
                        println!("[DEBUG] State response structure: {}", 
                            serde_json::to_string_pretty(&state_response).unwrap_or_else(|_| "Invalid JSON".to_string()));
                        
                        return Ok(state_response);
                    } else {
                        println!("[DEBUG] ===== NO INITIAL STATE RECEIVED FROM STREAM =====");
                        return Err("No initial state received from stream".into());
                    }
                }
                Err(e) => {
                    let error_msg = format!("Failed to establish state subscription: {}", e);
                    println!("[DEBUG] ===== STATE SUBSCRIPTION FAILED =====");
                    println!("[DEBUG] Error: {}", error_msg);
                    return Err(error_msg.into());
                }
            }
        } else {
            println!("[DEBUG] ===== NO STATESERVICE CLIENT AVAILABLE =====");
            Err("No StateService gRPC client available".into())
        }
    }
    
    async fn handle_streaming_state(
        &mut self, 
        mut stream: tonic::Streaming<crate::grpc_client::cline::State>,
        config: StreamConfig
    ) -> GrpcResult<Value> {
        log_debug("Starting streaming state processing");
        
        let mut message_count = 0;
        let max_messages = config.max_messages.unwrap_or(usize::MAX);
        let mut last_state: Option<Value> = None;
        
        while let Some(state_result) = stream.message().await? {
            let state_value: Value = serde_json::from_str(&state_result.state_json)
                .unwrap_or_else(|e| {
                    log_debug(&format!("Failed to parse state_json: {}, using raw string", e));
                    serde_json::json!({ "state_json": state_result.state_json })
                });
            
            // 如果有回调，调用它
            if let Some(ref callback) = config.callback {
                if let Err(e) = callback(state_value.clone()) {
                    log_debug(&format!("Stream callback error: {}", e));
                }
            }
            
            last_state = Some(state_value);
            message_count += 1;
            
            log_debug(&format!("Processed streaming state message {}/{}", message_count, max_messages));
            
            // 检查是否达到最大消息数量
            if message_count >= max_messages {
                log_debug("Reached maximum message limit, stopping stream");
                break;
            }
        }
        
        log_success(&format!("Streaming state processing completed, processed {} messages", message_count));
        
        // 返回最后一条消息或默认值
        Ok(last_state.unwrap_or_else(|| serde_json::json!({
            "streaming": true,
            "messages_processed": message_count
        })))
    }
    
    // 静态方法：在后台处理状态流式数据（带配置）
    async fn handle_background_state_stream(
        mut stream: tonic::Streaming<crate::grpc_client::cline::State>,
        config: StreamConfig
    ) -> GrpcResult<()> {
        println!("[DEBUG] Starting background state stream processing with config");
        
        let mut message_count = 0;
        let max_messages = config.max_messages.unwrap_or(usize::MAX);
        
        while let Some(state_result) = stream.message().await.map_err(|e| {
            println!("[DEBUG] Background state stream error: {}", e);
            format!("Background state stream error: {}", e)
        })? {
            message_count += 1;
            
            println!("[DEBUG] ===== RECEIVED STATE UPDATE #{} IN BACKGROUND =====", message_count);
            println!("[DEBUG] State JSON length: {}", state_result.state_json.len());
            println!("[DEBUG] State JSON preview: {}", 
                if state_result.state_json.len() > 200 { 
                    &state_result.state_json[..200] 
                } else { 
                    &state_result.state_json 
                });
            
            // 构建状态值
            let state_value = serde_json::json!({
                "stateJson": state_result.state_json
            });
            
            // 如果有回调，调用它来转发状态更新到前端
            if let Some(ref callback) = config.callback {
                if let Err(e) = callback(state_value) {
                    println!("[DEBUG] Background state stream callback error: {}", e);
                }
            }
            
            println!("[DEBUG] Processed background state update {}/{}", message_count, max_messages);
            
            // 检查是否达到最大消息数量
            if message_count >= max_messages {
                println!("[DEBUG] Reached maximum message limit in background state stream");
                break;
            }
        }
        
        log_success(&format!("Background state stream processing completed, processed {} updates", message_count));
        Ok(())
    }
    
    // 静态方法：处理默认的状态流式数据（类似 UiService 的处理方式）
    async fn handle_default_state_stream(
        mut stream: tonic::Streaming<crate::grpc_client::cline::State>
    ) -> GrpcResult<()> {
        println!("[DEBUG] ===== Starting default state stream processing - maintaining active connection for real-time state updates =====");
        
        let mut message_count = 0;
        
        // 保持流连接活跃以接收实时的状态更新
        while let Some(state_result) = stream.message().await.map_err(|e| {
            println!("[DEBUG] Default state stream error: {}", e);
            format!("Default state stream error: {}", e)
        })? {
            message_count += 1;
            
            println!("[DEBUG] ===== RECEIVED STATE UPDATE #{} =====", message_count);
            println!("[DEBUG] State JSON length: {}", state_result.state_json.len());
            
            // 尝试解析状态 JSON 来提取关键信息
            if let Ok(parsed_state) = serde_json::from_str::<Value>(&state_result.state_json) {
                // 检查是否包含 API 配置更新
                if let Some(api_config) = parsed_state.get("apiConfiguration") {
                    println!("[DEBUG] ===== API CONFIGURATION UPDATE DETECTED =====");
                    println!("[DEBUG] API Config: {}", serde_json::to_string_pretty(api_config).unwrap_or_else(|_| "Invalid JSON".to_string()));
                }
                
                // 检查其他重要状态字段
                if let Some(models) = parsed_state.get("models") {
                    println!("[DEBUG] Models updated in state");
                }
                
                if let Some(provider) = parsed_state.get("apiProvider") {
                    println!("[DEBUG] API Provider in state: {}", provider);
                }
            } else {
                println!("[DEBUG] Could not parse state JSON, raw preview: {}", 
                    if state_result.state_json.len() > 300 { 
                        &state_result.state_json[..300] 
                    } else { 
                        &state_result.state_json 
                    });
            }
            
            // 这里是关键：需要将状态更新转发给前端
            // 在实际实现中，这里应该通过某种机制（如事件总线、回调等）
            // 将状态更新推送到前端，触发 UI 重新渲染
            println!("[DEBUG] State update #{} processed - should trigger frontend update", message_count);
        }
        
        log_success(&format!(
            "[StateService] Default state stream completed, processed {} state updates", 
            message_count
        ));
        Ok(())
    }
}

impl StateServiceHandler {
    pub async fn handle_request(&mut self, method: &str, _message: &Value) -> GrpcResult<Value> {
        self.handle_request_with_config(method, _message, None).await
    }
    
    pub async fn handle_request_with_config(&mut self, method: &str, _message: &Value, stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        match method {
            "getLatestState" => self.get_latest_state().await,
            "subscribeToState" => self.subscribe_to_state_with_config(stream_config).await,
            _ => {
                log_debug(&format!("StateService method not implemented: {}", method));
                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("StateService method {} not implemented yet", method)
                }))
            }
        }
    }
}