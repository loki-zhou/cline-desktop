use tonic::transport::Channel;
use tonic::Request;
use serde_json::Value;
use tauri::WebviewWindow;
use std::sync::{Arc, Mutex};
use lazy_static::lazy_static;

use crate::grpc_client::{
    cline::{state_service_client::StateServiceClient, EmptyRequest, Metadata},
    types::{GrpcResult, StreamConfig},
    utils::{with_timeout, log_debug, log_success, DEFAULT_REQUEST_TIMEOUT},
};

// 🔥 全局状态订阅管理器
type StateSubscriptionManager = Arc<Mutex<Option<(String, WebviewWindow)>>>;

lazy_static! {
    static ref GLOBAL_STATE_SUBSCRIPTION: StateSubscriptionManager = Arc::new(Mutex::new(None));
}

// 🔥 设置全局状态订阅的 request_id 和 window
pub fn set_global_state_subscription(request_id: String, window: WebviewWindow) {
    if let Ok(mut subscription) = GLOBAL_STATE_SUBSCRIPTION.lock() {
        println!("[DEBUG] 🔥 Global state subscription set: request_id={}", request_id);
        *subscription = Some((request_id, window));
    }
}

// 🔥 获取全局状态订阅信息
pub fn get_global_state_subscription() -> Option<(String, WebviewWindow)> {
    if let Ok(subscription) = GLOBAL_STATE_SUBSCRIPTION.lock() {
        subscription.as_ref().cloned()
    } else {
        None
    }
}

#[derive(Debug)]
pub struct StateServiceHandler {
    client: Option<StateServiceClient<Channel>>,
    window: Option<WebviewWindow>,
}

impl StateServiceHandler {
    pub fn new() -> Self {
        Self { 
            client: None,
            window: None,
        }
    }
    
    pub fn set_client(&mut self, channel: Channel) {
        self.client = Some(StateServiceClient::new(channel));
    }
    
    pub fn set_window(&mut self, window: WebviewWindow) {
        self.window = Some(window);
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
            
            println!("[DEBUG] Successfully got stream from cline-core, setting up persistent streaming...");
            
            // 🔥 修复关键问题：总是启用流式处理以监听状态更新
            let effective_config = stream_config.unwrap_or_else(|| {
                println!("[DEBUG] No stream config provided, creating default streaming config");
                StreamConfig {
                    enable_streaming: true,  // 🔥 关键修复：总是启用流式处理
                    callback: None,
                    max_messages: None,      // 🔥 无限制监听
                }
            });
            
            // 🔥 总是使用流式处理来监听状态更新
            return self.handle_streaming_state(stream, effective_config).await;
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
        log_debug("🔥 Starting PERSISTENT streaming state processing for ongoing updates");
        
        let mut message_count = 0;
        let max_messages = config.max_messages.unwrap_or(usize::MAX);
        let mut first_state: Option<Value> = None;
        
        // 🔥 在后台启动持续监听任务
        if let Some(window) = self.window.clone() {
            println!("[DEBUG] 🔥 Starting background task for persistent state monitoring");
            
            let window_clone = window.clone();
            tokio::spawn(async move {
                println!("[DEBUG] 🔥 Background state monitoring task started");
                
                // 🔥 保存初始 request_id 以用于最初响应
                let mut original_request_id: Option<String> = None;
                
                loop {
                    match stream.message().await {
                        Ok(Some(state_result)) => {
                            println!("[DEBUG] 🔥 ===== RECEIVED STREAMING STATE UPDATE (Background) =====");
                            println!("[DEBUG] State update content: {}", state_result.state_json);

                            // 构造状态响应
                            let state_response = serde_json::json!({
                                "stateJson": state_result.state_json
                            });
                            
                            // 🔥 使用全局保存的 request_id 或者生成新的
                            let request_id = if let Some((saved_request_id, _)) = get_global_state_subscription() {
                                saved_request_id
                            } else {
                                let new_id = "state_subscription_stream".to_string();
                                original_request_id = Some(new_id.clone());
                                new_id
                            };
                            
                            // 实时转发状态更新到前端
                            let response_message = serde_json::json!({
                                "type": "grpc_response",
                                "grpc_response": {
                                    "request_id": request_id,
                                    "message": state_response,
                                    "error": null,
                                    "is_streaming": true
                                }
                            });
                            
                            // 使用 eval 执行 JavaScript 将响应发送到前端
                            let js_code = format!(
                                "window.dispatchEvent(new MessageEvent('message', {{ data: {} }}));",
                                response_message.to_string()
                            );
                            
                            match window_clone.eval(&js_code) {
                                Ok(_) => println!("[DEBUG] 🔥 ✅ State update forwarded to frontend successfully"),
                                Err(e) => println!("[DEBUG] 🔥 ❌ Failed to forward state update: {}", e),
                            }
                        }
                        Ok(None) => {
                            println!("[DEBUG] 🔥 State stream ended, no more updates");
                            break;
                        }
                        Err(e) => {
                            println!("[DEBUG] 🔥 ❌ State stream error: {}", e);
                            break;
                        }
                    }
                }
                
                println!("[DEBUG] 🔥 Background state monitoring task ended");
            });
            
            // 🔥 立即返回初始状态，让前端先水合
            println!("[DEBUG] 🔥 Returning initial success response to allow frontend hydration");
            Ok(serde_json::json!({
                "streaming": true,
                "background_monitoring": true,
                "message": "State subscription active, monitoring for updates in background"
            }))
        } else {
            // 如果没有窗口引用，使用同步处理方式
            println!("[DEBUG] 🔥 No window reference, using synchronous streaming");
            
            while let Some(state_result) = stream.message().await? {
                let state_response = serde_json::json!({
                    "stateJson": state_result.state_json
                });
                
                println!("[DEBUG] 🔥 ===== RECEIVED STREAMING STATE UPDATE (Sync) =====");
                log_success(&format!("Received streaming state update, state_json length: {}", 
                    state_result.state_json.len()));
                
                // 记录第一个状态用于返回
                if first_state.is_none() {
                    first_state = Some(state_response.clone());
                }
                
                message_count += 1;
                log_debug(&format!("Processed streaming state message {}/{}", message_count, max_messages));
                
                // 检查是否达到最大消息数量
                if message_count >= max_messages {
                    log_debug("Reached maximum message limit, stopping stream");
                    break;
                }
            }
            
            log_success(&format!("Streaming state processing completed, processed {} messages", message_count));
            
            // 返回第一条消息或默认值
            Ok(first_state.unwrap_or_else(|| serde_json::json!({
                "streaming": true,
                "messages_processed": message_count
            })))
        }
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
