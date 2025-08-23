use tonic::transport::Channel;
use tonic::Request;
use serde_json::Value;

use crate::grpc_client::{
    cline::{ui_service_client::UiServiceClient, EmptyRequest, Metadata},
    types::{GrpcResult, StreamConfig},
    utils::{with_timeout, log_debug, log_success, log_error, DEFAULT_REQUEST_TIMEOUT},
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
    
    pub async fn handle_request(&mut self, method: &str, message: &Value) -> GrpcResult<Value> {
        // 对于订阅方法，传递空的配置以使用被动订阅模式
        if matches!(method, "subscribeToPartialMessage") {
            // 对于订阅服务，不启用流式处理，立即返回成功
            self.handle_request_with_config(method, message, None).await
        } else {
            self.handle_request_with_config(method, message, None).await
        }
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
        log_debug("[UiService] Starting subscribeToPartialMessage with new logic");
        
        if let Some(client) = &mut self.client {
            log_debug("Calling subscribeToPartialMessage on cline-core");
            
            let request = Request::new(EmptyRequest {
                metadata: Some(Metadata {}),
            });
            
            // 建立流式连接
            let stream_result = with_timeout(
                client.subscribe_to_partial_message(request),
                DEFAULT_REQUEST_TIMEOUT,
                "subscribeToPartialMessage connection"
            ).await?;
            
            let mut stream = stream_result.into_inner();
            
            log_success("[UiService] Successfully established partial message subscription - returning immediately");
            
            // 对于被动订阅服务，立即返回成功响应
            // 不等待数据推送，因为这是事件驱动的
            if let Some(config) = stream_config {
                if config.enable_streaming {
                    // 如果明确启用了流式处理，在后台异步处理
                    log_debug("[UiService] Starting background stream processing");
                    tokio::spawn(async move {
                        let _ = Self::handle_background_partial_messages_stream(stream, config).await;
                    });
                }
            } else {
                log_debug("[UiService] No stream config - dropping stream connection");
            }
            
            // 立即返回订阅成功状态
            let success_response = serde_json::json!({
                "subscription_established": true,
                "message": "Successfully subscribed to partial messages",
                "type": "subscription",
                "service": "UiService",
                "method": "subscribeToPartialMessage"
            });
            
            log_success(&format!("[UiService] Returning success response: {}", success_response));
            return Ok(success_response);
        } else {
            let error_msg = "No UiService gRPC client available";
            log_error(&format!("[UiService] {}", error_msg));
            Err(error_msg.into())
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
    
    // 静态方法：构造部分消息响应（用于后台处理）
    fn build_static_partial_message_response(message_result: &crate::grpc_client::cline::ClineMessage) -> Value {
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
    
    // 静态方法：在后台处理部分消息流式数据
    async fn handle_background_partial_messages_stream(
        mut stream: tonic::Streaming<crate::grpc_client::cline::ClineMessage>,
        config: StreamConfig
    ) -> GrpcResult<()> {
        log_debug("Starting background partial messages stream processing");
        
        let mut message_count = 0;
        let max_messages = config.max_messages.unwrap_or(usize::MAX);
        
        while let Some(message_result) = stream.message().await.map_err(|e| {
            format!("Stream error: {}", e)
        })? {
            let message_value = Self::build_static_partial_message_response(&message_result);
            
            // 如果有回调，调用它
            if let Some(ref callback) = config.callback {
                if let Err(e) = callback(message_value) {
                    log_debug(&format!("Background stream callback error: {}", e));
                }
            }
            
            message_count += 1;
            log_debug(&format!("Processed background partial message {}/{}", message_count, max_messages));
            
            // 检查是否达到最大消息数量
            if message_count >= max_messages {
                log_debug("Reached maximum message limit in background stream");
                break;
            }
        }
        
        log_success(&format!("Background partial messages stream processing completed, processed {} messages", message_count));
        Ok(())
    }
}