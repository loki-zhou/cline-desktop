use serde_json::Value;
use tonic::transport::Channel;
use tonic::Request;
use crate::grpc_client::{
    cline::{models_service_client::ModelsServiceClient, UpdateApiConfigurationRequest, ModelsApiConfiguration, Metadata},
    types::{GrpcResult, StreamConfig},
    utils::{log_debug, log_success, log_error, with_timeout, DEFAULT_REQUEST_TIMEOUT},
};

#[derive(Debug)]
pub struct ModelsServiceHandler {
    client: Option<ModelsServiceClient<Channel>>,
}

impl ModelsServiceHandler {
    pub fn new() -> Self {
        Self { client: None }
    }

    pub fn set_client(&mut self, channel: Channel) {
        self.client = Some(ModelsServiceClient::new(channel));
    }

    async fn update_api_configuration_proto(&mut self, message: &Value) -> GrpcResult<Value> {
        if let Some(client) = &mut self.client {
            log_debug("Calling updateApiConfigurationProto on cline-core");
            
            // 先解析 JSON 消息到 protobuf 结构（在借用之前）
            let api_config = Self::parse_api_configuration_from_json_static(message)?;
            
            let request = Request::new(UpdateApiConfigurationRequest {
                metadata: Some(Metadata {}),
                api_configuration: Some(api_config),
            });
            
            let response = with_timeout(
                client.update_api_configuration_proto(request),
                DEFAULT_REQUEST_TIMEOUT,
                "updateApiConfigurationProto"
            ).await?;
            
            let _result = response.into_inner();
            log_success("API configuration updated successfully in cline-core");
            
            Ok(serde_json::json!({
                "success": true
            }))
        } else {
            Err("No ModelsService gRPC client available".into())
        }
    }
    
    fn parse_api_configuration_from_json_static(message: &Value) -> GrpcResult<ModelsApiConfiguration> {
        log_debug(&format!("Parsing API configuration from JSON: {}", message));
        
        // 从 JSON 中提取字段并转换为枚举值
        let plan_mode_api_provider = message.get("planModeApiProvider")
            .and_then(|v| v.as_str())
            .map(|s| Self::string_to_api_provider_static(s));
            
        let act_mode_api_provider = message.get("actModeApiProvider")
            .and_then(|v| v.as_str())
            .map(|s| Self::string_to_api_provider_static(s));
        
        log_debug(&format!("Parsed API providers - plan: {:?}, act: {:?}", 
            plan_mode_api_provider, act_mode_api_provider));
        
        // 创建一个基本的 ModelsApiConfiguration，只设置必要的字段
        Ok(ModelsApiConfiguration {
            plan_mode_api_provider,
            act_mode_api_provider,
            ..Default::default()
        })
    }
    
    fn string_to_api_provider_static(provider: &str) -> i32 {
        match provider.to_lowercase().as_str() {
            "anthropic" => 0,
            "openrouter" => 1,
            "bedrock" => 2,
            "vertex" => 3,
            "openai" => 4,
            "ollama" => 5,
            "lmstudio" => 6,
            "gemini" => 7,
            "openai_native" => 8,
            "requesty" => 9,
            "together" => 10,
            "deepseek" => 11,
            "qwen" => 12,
            "doubao" => 13,
            "mistral" => 14,
            "vscode_lm" => 15,
            "cline" => 16,
            "litellm" => 17,
            "nebius" => 18,
            "fireworks" => 19,
            "asksage" => 20,
            "xai" => 21,
            "sambanova" => 22,
            "cerebras" => 23,
            "groq" => 24,
            "sapaicore" => 25,
            "claude_code" => 26,
            "moonshot" => 27,
            "huggingface" => 28,
            "huawei_cloud_maas" => 29,
            "baseten" => 30,
            "zai" => 31,
            "vercel_ai_gateway" => 32,
            _ => 0, // 默认为 ANTHROPIC
        }
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
        match method {
            "updateApiConfigurationProto" => {
                log_debug("Processing updateApiConfigurationProto request");
                self.update_api_configuration_proto(_message).await
            }
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
                log_debug(&format!("ModelsService method not implemented: {}", method));
                Ok(serde_json::json!({
                    "models": [],
                    "success": true
                }))
            }
        }
    }
}