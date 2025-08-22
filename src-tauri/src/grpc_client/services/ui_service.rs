use tonic::transport::Channel;
use tonic::Request;
use serde_json::Value;

use crate::grpc_client::{
    cline::{ui_service_client::UiServiceClient, EmptyRequest, Metadata},
    types::{GrpcResult, StreamConfig},
    utils::{with_timeout, log_debug, log_success, DEFAULT_REQUEST_TIMEOUT},
};

#[derive(Debug)]
pub struct UiServiceHandler {
    client: Option<UiServiceClient<Channel>>,
}

impl UiServiceHandler {
    pub fn new() -> Self {
        Self { client: None }
    }
    
    pub fn set_client(&mut self, channel: Channel) {
        self.client = Some(UiServiceClient::new(channel));
    }
    
    pub async fn handle_request(&mut self, method: &str, _message: &Value) -> GrpcResult<Value> {
        self.handle_request_with_config(method, _message, None).await
    }
    
    pub async fn handle_request_with_config(&mut self, method: &str, _message: &Value, stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        match method {
            "subscribeToPartialMessage" => self.subscribe_to_partial_message_with_config(stream_config).await,
            _ => {
                log_debug(&format!("UiService method not implemented: {}", method));
                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("UiService method {} not implemented yet", method)
                }))
            }
        }
    }
    
    async fn subscribe_to_partial_message(&mut self) -> GrpcResult<Value> {
        self.subscribe_to_partial_message_with_config(None).await
    }
    
    async fn subscribe_to_partial_message_with_config(&mut self, stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        if let Some(client) = &mut self.client {
            log_debug("Calling subscribeToPartialMessage on cline-core");
            
            let request = Request::new(EmptyRequest {
                metadata: Some(Metadata {}),
            });
            
            let mut stream = with_timeout(
                client.subscribe_to_partial_message(request),
                DEFAULT_REQUEST_TIMEOUT,
                "subscribeToPartialMessage"
            ).await?.into_inner();
            
            // 如果配置了流式处理，则持续监听
            if let Some(config) = stream_config {
                if config.enable_streaming {
                    return self.handle_streaming_partial_messages(stream, config).await;
                }
            }
            
            // 默认行为：只返回第一个响应
            if let Some(message_result) = stream.message().await? {
                log_success("Received partial message from subscribeToPartialMessage");
                
                let message_value = self.build_partial_message_response(&message_result);
                return Ok(message_value);
            }
            
            Err("No partial message received from stream".into())
        } else {
            Err("No UiService gRPC client available".into())
        }
    }
    
    async fn handle_streaming_partial_messages(
        &mut self, 
        mut stream: tonic::Streaming<crate::grpc_client::cline::ClineMessage>,
        config: StreamConfig
    ) -> GrpcResult<Value> {
        log_debug("Starting streaming partial message processing");
        
        let mut message_count = 0;
        let max_messages = config.max_messages.unwrap_or(usize::MAX);
        let mut last_message: Option<Value> = None;
        
        while let Some(message_result) = stream.message().await? {
            let message_value = self.build_partial_message_response(&message_result);
            
            // 如果有回调，调用它
            if let Some(ref callback) = config.callback {
                if let Err(e) = callback(message_value.clone()) {
                    log_debug(&format!("Stream callback error: {}", e));
                }
            }
            
            last_message = Some(message_value);
            message_count += 1;
            
            log_debug(&format!("Processed streaming partial message {}/{}", message_count, max_messages));
            
            // 检查是否达到最大消息数量
            if message_count >= max_messages {
                log_debug("Reached maximum message limit, stopping stream");
                break;
            }
        }
        
        log_success(&format!("Streaming partial message processing completed, processed {} messages", message_count));
        
        // 返回最后一条消息或默认值
        Ok(last_message.unwrap_or_else(|| serde_json::json!({
            "streaming": true,
            "messages_processed": message_count
        })))
    }
    
    // 辅助方法：构造部分消息响应
    fn build_partial_message_response(&self, message_result: &crate::grpc_client::cline::ClineMessage) -> Value {
        serde_json::json!({
            "ts": message_result.ts,
            "type": message_result.r#type,
            "ask": message_result.ask,
            "say": message_result.say,
            "text": message_result.text,
            "reasoning": message_result.reasoning,
            "images": message_result.images,
            "files": message_result.files,
            "partial": message_result.partial,
            "lastCheckpointHash": message_result.last_checkpoint_hash,
            "isCheckpointCheckedOut": message_result.is_checkpoint_checked_out,
            "isOperationOutsideWorkspace": message_result.is_operation_outside_workspace,
            "conversationHistoryIndex": message_result.conversation_history_index
        })
    }
}