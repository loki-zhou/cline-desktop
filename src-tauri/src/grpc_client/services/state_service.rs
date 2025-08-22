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
            
            // 解析 state_json
            let state_value: Value = serde_json::from_str(&state.state_json)
                .unwrap_or_else(|e| {
                    log_debug(&format!("Failed to parse state_json: {}, using raw string", e));
                    serde_json::json!({ "state_json": state.state_json })
                });
            
            Ok(state_value)
        } else {
            Err("No StateService gRPC client available".into())
        }
    }
    
    async fn subscribe_to_state(&mut self) -> GrpcResult<Value> {
        self.subscribe_to_state_with_config(None).await
    }
    
    async fn subscribe_to_state_with_config(&mut self, stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        if let Some(client) = &mut self.client {
            log_debug("Calling subscribeToState on cline-core");
            
            let request = Request::new(EmptyRequest {
                metadata: Some(Metadata {}),
            });
            
            let mut stream = with_timeout(
                client.subscribe_to_state(request),
                DEFAULT_REQUEST_TIMEOUT,
                "subscribeToState"
            ).await?.into_inner();
            
            // 如果配置了流式处理，则持续监听
            if let Some(config) = stream_config {
                if config.enable_streaming {
                    return self.handle_streaming_state(stream, config).await;
                }
            }
            
            // 默认行为：只返回第一个响应
            if let Some(state_result) = stream.message().await? {
                log_success(&format!("Received state from subscribeToState, state_json length: {}", 
                    state_result.state_json.len()));
                
                let state_value: Value = serde_json::from_str(&state_result.state_json)
                    .unwrap_or_else(|e| {
                        log_debug(&format!("Failed to parse state_json: {}, using raw string", e));
                        serde_json::json!({ "state_json": state_result.state_json })
                    });
                
                return Ok(state_value);
            }
            
            Err("No state received from stream".into())
        } else {
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