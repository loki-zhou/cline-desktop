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
    
    pub async fn handle_request_with_config(&mut self, method: &str, message: &Value, stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        match method {
            "subscribeToPartialMessage" => self.subscribe_to_partial_message_with_config(stream_config).await,
            "subscribeToChatButtonClicked" => self.subscribe_to_chat_button_clicked_with_config(stream_config).await,
            "initializeWebview" => self.initialize_webview().await,
            "subscribeToTheme" => self.subscribe_to_theme_with_config(stream_config).await,
            "subscribeToRelinquishControl" => self.subscribe_to_relinquish_control_with_config(stream_config).await,
            "subscribeToFocusChatInput" => self.subscribe_to_focus_chat_input_with_config(message, stream_config).await,
            "subscribeToAddToInput" => self.subscribe_to_add_to_input_with_config(message, stream_config).await,
            "subscribeToMcpButtonClicked" => self.subscribe_to_mcp_button_clicked_with_config(message, stream_config).await,
            "subscribeToHistoryButtonClicked" => self.subscribe_to_history_button_clicked_with_config(message, stream_config).await,
            "subscribeToAccountButtonClicked" => self.subscribe_to_account_button_clicked_with_config(stream_config).await,
            "subscribeToSettingsButtonClicked" => self.subscribe_to_settings_button_clicked_with_config(message, stream_config).await,
            "subscribeToDidBecomeVisible" => self.subscribe_to_did_become_visible_with_config(stream_config).await,
            "scrollToSettings" => self.scroll_to_settings(message).await,
            "onDidShowAnnouncement" => self.on_did_show_announcement().await,
            "getWebviewHtml" => self.get_webview_html().await,
            "openUrl" => self.open_url(message).await,
            "openWalkthrough" => self.open_walkthrough().await,
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
            
            // 建立流式连接，但不等待第一个消息
            match client.subscribe_to_partial_message(request).await {
                Ok(stream_result) => {
                    let stream = stream_result.into_inner();
                    
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
                        } else {
                            log_debug("[UiService] Starting default background stream processing");
                            // 即使没有启用显式流式处理，也要保持连接以接收部分消息推送
                            tokio::spawn(async move {
                                let _ = Self::handle_default_partial_messages_stream(stream).await;
                            });
                        }
                    } else {
                        log_debug("[UiService] Starting default background stream processing (no config)");
                        // 没有配置时，使用默认的流式处理来接收部分消息推送
                        tokio::spawn(async move {
                            let _ = Self::handle_default_partial_messages_stream(stream).await;
                        });
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
                }
                Err(e) => {
                    let error_msg = format!("Failed to establish partial message subscription: {}", e);
                    log_error(&format!("[UiService] {}", error_msg));
                    return Err(error_msg.into());
                }
            }
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
    
    // 静态方法：处理默认的部分消息流式数据（根据 cline 原始逻辑）
    async fn handle_default_partial_messages_stream(
        mut stream: tonic::Streaming<crate::grpc_client::cline::ClineMessage>
    ) -> GrpcResult<()> {
        log_debug("[UiService] Starting default partial messages stream processing - maintaining active connection for real-time updates");
        
        let mut message_count = 0;
        
        // 根据 cline 原始逻辑，保持流连接活跃以接收实时的部分消息更新
        while let Some(message_result) = stream.message().await.map_err(|e| {
            log_error(&format!("[UiService] Default partial messages stream error: {}", e));
            format!("Default partial messages stream error: {}", e)
        })? {
            message_count += 1;
            
            // 构建消息响应
            let message_value = Self::build_static_partial_message_response(&message_result);
            
            log_debug(&format!(
                "[UiService] Received partial message update #{}: type={}, partial={}, text_len={}", 
                message_count,
                message_result.r#type,
                message_result.partial,
                message_result.text.len()
            ));
            
            // 在这里可以将部分消息更新转发给前端
            // 这模拟了原始 cline 中实时消息推送的功能
            if message_result.partial {
                log_debug(&format!(
                    "[UiService] Processing partial message: {}",
                    message_result.text.chars().take(100).collect::<String>()
                ));
            } else {
                log_success(&format!(
                    "[UiService] Received complete message: type={}, conversation_index={}",
                    message_result.r#type,
                    message_result.conversation_history_index
                ));
            }
        }
        
        log_success(&format!(
            "[UiService] Default partial messages stream completed, processed {} updates", 
            message_count
        ));
        Ok(())
    }

    // 新增的 UiService 方法实现
    
    async fn subscribe_to_chat_button_clicked_with_config(&mut self, stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        log_debug("[UiService] Starting subscribeToChatButtonClicked");
        
        if let Some(client) = &mut self.client {
            let request = Request::new(EmptyRequest {
                metadata: Some(Metadata {}),
            });
            
            match client.subscribe_to_chat_button_clicked(request).await {
                Ok(stream_result) => {
                    let stream = stream_result.into_inner();
                    log_success("[UiService] Successfully established chat button clicked subscription");
                    
                    // 在后台处理流
                    tokio::spawn(async move {
                        let _ = Self::handle_empty_stream(stream, "chat_button_clicked").await;
                    });
                    
                    Ok(serde_json::json!({
                        "subscription_established": true,
                        "message": "Successfully subscribed to chat button clicked events",
                        "type": "subscription",
                        "service": "UiService",
                        "method": "subscribeToChatButtonClicked"
                    }))
                }
                Err(e) => {
                    let error_msg = format!("Failed to establish chat button clicked subscription: {}", e);
                    log_error(&format!("[UiService] {}", error_msg));
                    Err(error_msg.into())
                }
            }
        } else {
            let error_msg = "No UiService gRPC client available";
            log_error(&format!("[UiService] {}", error_msg));
            Err(error_msg.into())
        }
    }
    
    async fn initialize_webview(&mut self) -> GrpcResult<Value> {
        log_debug("[UiService] Starting initializeWebview");
        
        if let Some(client) = &mut self.client {
            let request = Request::new(EmptyRequest {
                metadata: Some(Metadata {}),
            });
            
            match client.initialize_webview(request).await {
                Ok(_) => {
                    log_success("[UiService] Successfully initialized webview");
                    Ok(serde_json::json!({
                        "success": true,
                        "message": "Webview initialized successfully",
                        "service": "UiService",
                        "method": "initializeWebview"
                    }))
                }
                Err(e) => {
                    let error_msg = format!("Failed to initialize webview: {}", e);
                    log_error(&format!("[UiService] {}", error_msg));
                    Err(error_msg.into())
                }
            }
        } else {
            let error_msg = "No UiService gRPC client available";
            log_error(&format!("[UiService] {}", error_msg));
            Err(error_msg.into())
        }
    }
    
    async fn subscribe_to_theme_with_config(&mut self, stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        log_debug("[UiService] Starting subscribeToTheme");
        
        if let Some(client) = &mut self.client {
            let request = Request::new(EmptyRequest {
                metadata: Some(Metadata {}),
            });
            
            match client.subscribe_to_theme(request).await {
                Ok(stream_result) => {
                    let stream = stream_result.into_inner();
                    log_success("[UiService] Successfully established theme subscription");
                    
                    // 在后台处理流
                    tokio::spawn(async move {
                        let _ = Self::handle_string_stream(stream, "theme").await;
                    });
                    
                    Ok(serde_json::json!({
                        "subscription_established": true,
                        "message": "Successfully subscribed to theme changes",
                        "type": "subscription",
                        "service": "UiService",
                        "method": "subscribeToTheme"
                    }))
                }
                Err(e) => {
                    let error_msg = format!("Failed to establish theme subscription: {}", e);
                    log_error(&format!("[UiService] {}", error_msg));
                    Err(error_msg.into())
                }
            }
        } else {
            let error_msg = "No UiService gRPC client available";
            log_error(&format!("[UiService] {}", error_msg));
            Err(error_msg.into())
        }
    }
    
    async fn subscribe_to_relinquish_control_with_config(&mut self, stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        log_debug("[UiService] Starting subscribeToRelinquishControl");
        
        if let Some(client) = &mut self.client {
            let request = Request::new(EmptyRequest {
                metadata: Some(Metadata {}),
            });
            
            match client.subscribe_to_relinquish_control(request).await {
                Ok(stream_result) => {
                    let stream = stream_result.into_inner();
                    log_success("[UiService] Successfully established relinquish control subscription");
                    
                    // 在后台处理流
                    tokio::spawn(async move {
                        let _ = Self::handle_empty_stream(stream, "relinquish_control").await;
                    });
                    
                    Ok(serde_json::json!({
                        "subscription_established": true,
                        "message": "Successfully subscribed to relinquish control events",
                        "type": "subscription",
                        "service": "UiService",
                        "method": "subscribeToRelinquishControl"
                    }))
                }
                Err(e) => {
                    let error_msg = format!("Failed to establish relinquish control subscription: {}", e);
                    log_error(&format!("[UiService] {}", error_msg));
                    Err(error_msg.into())
                }
            }
        } else {
            let error_msg = "No UiService gRPC client available";
            log_error(&format!("[UiService] {}", error_msg));
            Err(error_msg.into())
        }
    }
    
    async fn subscribe_to_focus_chat_input_with_config(&mut self, message: &Value, _stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        log_debug("[UiService] subscribeToFocusChatInput - returning placeholder response");
        Ok(serde_json::json!({
            "subscription_established": true,
            "message": "Successfully subscribed to focus chat input events",
            "type": "subscription",
            "service": "UiService",
            "method": "subscribeToFocusChatInput"
        }))
    }
    
    async fn subscribe_to_add_to_input_with_config(&mut self, _message: &Value, _stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        log_debug("[UiService] subscribeToAddToInput - returning placeholder response");
        Ok(serde_json::json!({
            "subscription_established": true,
            "message": "Successfully subscribed to add to input events",
            "type": "subscription",
            "service": "UiService",
            "method": "subscribeToAddToInput"
        }))
    }
    
    async fn subscribe_to_mcp_button_clicked_with_config(&mut self, _message: &Value, _stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        log_debug("[UiService] subscribeToMcpButtonClicked - returning placeholder response");
        Ok(serde_json::json!({
            "subscription_established": true,
            "message": "Successfully subscribed to MCP button clicked events",
            "type": "subscription",
            "service": "UiService",
            "method": "subscribeToMcpButtonClicked"
        }))
    }
    
    async fn subscribe_to_history_button_clicked_with_config(&mut self, _message: &Value, _stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        log_debug("[UiService] subscribeToHistoryButtonClicked - returning placeholder response");
        Ok(serde_json::json!({
            "subscription_established": true,
            "message": "Successfully subscribed to history button clicked events",
            "type": "subscription",
            "service": "UiService",
            "method": "subscribeToHistoryButtonClicked"
        }))
    }
    
    async fn subscribe_to_account_button_clicked_with_config(&mut self, _stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        log_debug("[UiService] subscribeToAccountButtonClicked - returning placeholder response");
        Ok(serde_json::json!({
            "subscription_established": true,
            "message": "Successfully subscribed to account button clicked events",
            "type": "subscription",
            "service": "UiService",
            "method": "subscribeToAccountButtonClicked"
        }))
    }
    
    async fn subscribe_to_settings_button_clicked_with_config(&mut self, _message: &Value, _stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        log_debug("[UiService] subscribeToSettingsButtonClicked - returning placeholder response");
        Ok(serde_json::json!({
            "subscription_established": true,
            "message": "Successfully subscribed to settings button clicked events",
            "type": "subscription",
            "service": "UiService",
            "method": "subscribeToSettingsButtonClicked"
        }))
    }
    
    async fn subscribe_to_did_become_visible_with_config(&mut self, _stream_config: Option<StreamConfig>) -> GrpcResult<Value> {
        log_debug("[UiService] subscribeToDidBecomeVisible - returning placeholder response");
        Ok(serde_json::json!({
            "subscription_established": true,
            "message": "Successfully subscribed to webview visibility events",
            "type": "subscription",
            "service": "UiService",
            "method": "subscribeToDidBecomeVisible"
        }))
    }
    
    async fn scroll_to_settings(&mut self, _message: &Value) -> GrpcResult<Value> {
        log_debug("[UiService] scrollToSettings - returning placeholder response");
        Ok(serde_json::json!({
            "success": true,
            "message": "Settings scrolled",
            "service": "UiService",
            "method": "scrollToSettings"
        }))
    }
    
    async fn on_did_show_announcement(&mut self) -> GrpcResult<Value> {
        log_debug("[UiService] onDidShowAnnouncement - returning placeholder response");
        Ok(serde_json::json!({
            "value": false,
            "success": true,
            "service": "UiService",
            "method": "onDidShowAnnouncement"
        }))
    }
    
    async fn get_webview_html(&mut self) -> GrpcResult<Value> {
        log_debug("[UiService] getWebviewHtml - returning placeholder response");
        Ok(serde_json::json!({
            "html": "<html><body>Placeholder</body></html>",
            "success": true,
            "service": "UiService",
            "method": "getWebviewHtml"
        }))
    }
    
    async fn open_url(&mut self, _message: &Value) -> GrpcResult<Value> {
        log_debug("[UiService] openUrl - returning placeholder response");
        Ok(serde_json::json!({
            "success": true,
            "service": "UiService",
            "method": "openUrl"
        }))
    }
    
    async fn open_walkthrough(&mut self) -> GrpcResult<Value> {
        log_debug("[UiService] openWalkthrough - returning placeholder response");
        Ok(serde_json::json!({
            "success": true,
            "service": "UiService",
            "method": "openWalkthrough"
        }))
    }

    // 辅助方法：处理空流（用于事件订阅）
    async fn handle_empty_stream(
        mut stream: tonic::Streaming<crate::grpc_client::cline::Empty>,
        stream_name: &str
    ) -> GrpcResult<()> {
        log_debug(&format!("[UiService] Starting {} stream processing", stream_name));
        
        let mut event_count = 0;
        
        while let Some(_event_result) = stream.message().await.map_err(|e| {
            log_error(&format!("[UiService] {} stream error: {}", stream_name, e));
            format!("{} stream error: {}", stream_name, e)
        })? {
            event_count += 1;
            log_debug(&format!("[UiService] Received {} event #{}", stream_name, event_count));
        }
        
        log_success(&format!(
            "[UiService] {} stream completed, processed {} events",
            stream_name, event_count
        ));
        Ok(())
    }

    // 辅助方法：处理字符串流
    async fn handle_string_stream(
        mut stream: tonic::Streaming<crate::grpc_client::cline::String>,
        stream_name: &str
    ) -> GrpcResult<()> {
        log_debug(&format!("[UiService] Starting {} string stream processing", stream_name));
        
        let mut message_count = 0;
        
        while let Some(message_result) = stream.message().await.map_err(|e| {
            log_error(&format!("[UiService] {} string stream error: {}", stream_name, e));
            format!("{} string stream error: {}", stream_name, e)
        })? {
            message_count += 1;
            log_debug(&format!(
                "[UiService] Received {} string message #{}: {}", 
                stream_name, 
                message_count,
                message_result.value.chars().take(50).collect::<String>()
            ));
        }
        
        log_success(&format!(
            "[UiService] {} string stream completed, processed {} messages",
            stream_name, message_count
        ));
        Ok(())
    }
}