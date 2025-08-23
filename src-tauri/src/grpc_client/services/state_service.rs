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
            let mut stream = with_timeout(
                client.subscribe_to_state(request),
                DEFAULT_REQUEST_TIMEOUT,
                "subscribeToState"
            ).await?.into_inner();
            
            println!("[DEBUG] Successfully got stream from cline-core, checking configuration...");
            
            // 如果配置了流式处理，则持续监听
            if let Some(config) = stream_config {
                if config.enable_streaming {
                    return self.handle_streaming_state(stream, config).await;
                }
            }
            
            // 默认行为：只返回第一个响应
            println!("[DEBUG] Waiting for first message from stream...");
            if let Some(state_result) = stream.message().await? {
                println!("[DEBUG] ===== RECEIVED STATE FROM CLINE-CORE =====");
                log_success(&format!("Received state from subscribeToState, state_json length: {}", 
                    state_result.state_json.len()));
                println!("[DEBUG] Raw state_json (first 200 chars): {}", 
                    if state_result.state_json.len() > 200 { 
                        &state_result.state_json[..200] 
                    } else { 
                        &state_result.state_json 
                    });
                
                // 返回正确的 State 消息结构，保持 stateJson 字段
                let state_response = serde_json::json!({
                    "stateJson": state_result.state_json
                });
                
                println!("[DEBUG] ===== RETURNING STATE RESPONSE TO FRONTEND =====");
                println!("[DEBUG] State response structure: {}", 
                    serde_json::to_string_pretty(&state_response).unwrap_or_else(|_| "Invalid JSON".to_string()));
                
                return Ok(state_response);
            }
            
            println!("[DEBUG] ===== NO STATE RECEIVED FROM STREAM =====");
            Err("No state received from stream".into())
        } else {
            println!("[DEBUG] ===== NO STATSERVICE CLIENT AVAILABLE =====");
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