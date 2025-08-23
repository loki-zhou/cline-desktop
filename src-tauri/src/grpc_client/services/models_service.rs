use serde_json::Value;
use tonic::transport::Channel;
use crate::grpc_client::{
    types::{GrpcResult, StreamConfig},
    utils::log_debug,
};

#[derive(Debug)]
pub struct ModelsServiceHandler {
    client: Option<Channel>,
}

impl ModelsServiceHandler {
    pub fn new() -> Self {
        Self { client: None }
    }

    pub fn set_client(&mut self, channel: Channel) {
        self.client = Some(channel);
    }

    pub async fn handle_request(&mut self, method: &str, _message: &Value) -> GrpcResult<Value> {
        self.handle_request_with_config(method, _message, None).await
    }

    pub async fn handle_request_with_config(
        &mut self, 
        method: &str, 
        _message: &Value, 
        _stream_config: Option<StreamConfig>
    ) -> GrpcResult<Value> {
        log_debug(&format!("ModelsService method not implemented: {}", method));
        
        // 为不同的方法返回相应的默认值
        match method {
            "subscribeToOpenRouterModels" => {
                Ok(serde_json::json!({
                    "models": [],
                    "lastUpdated": chrono::Utc::now().timestamp_millis()
                }))
            }
            "getOllamaModels" => {
                Ok(serde_json::json!({
                    "models": []
                }))
            }
            "getLmStudioModels" => {
                Ok(serde_json::json!({
                    "models": []
                }))
            }
            "getVsCodeLmModels" => {
                Ok(serde_json::json!({
                    "models": []
                }))
            }
            "refreshOpenRouterModels" => {
                Ok(serde_json::json!({
                    "models": [],
                    "success": true
                }))
            }
            "refreshHuggingFaceModels" => {
                Ok(serde_json::json!({
                    "models": [],
                    "success": true
                }))
            }
            "refreshOpenAiModels" => {
                Ok(serde_json::json!({
                    "models": [],
                    "success": true
                }))
            }
            "refreshVercelAiGatewayModels" => {
                Ok(serde_json::json!({
                    "models": [],
                    "success": true
                }))
            }
            "refreshRequestyModels" => {
                Ok(serde_json::json!({
                    "models": [],
                    "success": true
                }))
            }
            "updateApiConfigurationProto" => {
                Ok(serde_json::json!({
                    "success": true
                }))
            }
            "refreshGroqModels" => {
                Ok(serde_json::json!({
                    "models": [],
                    "success": true
                }))
            }
            "refreshBasetenModels" => {
                Ok(serde_json::json!({
                    "models": [],
                    "success": true
                }))
            }
            "getSapAiCoreModels" => {
                Ok(serde_json::json!({
                    "models": []
                }))
            }
            _ => {
                log_debug(&format!("Unknown ModelsService method: {}", method));
                Ok(serde_json::json!({
                    "models": [],
                    "success": true
                }))
            }
        }
    }
}