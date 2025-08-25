use serde_json::Value;
use tonic::transport::Channel;
use tonic::Request;
use crate::grpc_client::{
    types::{GrpcResult, StreamConfig},
    utils::log_debug,
};

// Import the generated protobuf types
mod cline {
    tonic::include_proto!("cline");
}

use cline::{
    models_service_client::ModelsServiceClient,
    UpdateApiConfigurationRequest,
    ModelsApiConfiguration,
    ApiProvider,
    Metadata,
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
        message: &Value, 
        _stream_config: Option<StreamConfig>
    ) -> GrpcResult<Value> {
        log_debug(&format!("ModelsService handling method: {}", method));
        
        // 尝试真正的 gRPC 转发，特别是 updateApiConfigurationProto
        match method {
            "updateApiConfigurationProto" => {
                self.handle_update_api_configuration_proto(message).await
            }
            _ => {
                // 对于其他方法，返回模拟响应
                self.handle_other_methods(method).await
            }
        }
    }
    
    async fn handle_update_api_configuration_proto(&mut self, message: &Value) -> GrpcResult<Value> {
        log_debug(&format!("Processing updateApiConfigurationProto with message: {}", message));
        
        // 尝试建立 gRPC 连接
        if let Some(channel) = &self.client {
            let mut client = ModelsServiceClient::new(channel.clone());
            
            // 解析 JSON 数据并构造 protobuf 请求
            match self.parse_api_configuration(message) {
                Ok(api_config) => {
                    let request = Request::new(UpdateApiConfigurationRequest {
                        metadata: Some(Metadata::default()),
                        api_configuration: Some(api_config),
                    });
                    
                    log_debug("Sending gRPC updateApiConfigurationProto request...");
                    
                    // 发送 gRPC 请求
                    match client.update_api_configuration_proto(request).await {
                        Ok(response) => {
                            log_debug("✅ updateApiConfigurationProto gRPC request successful");
                            Ok(serde_json::json!({ "success": true }))
                        }
                        Err(e) => {
                            log_debug(&format!("❌ gRPC updateApiConfigurationProto failed: {}", e));
                            Err(format!("gRPC request failed: {}", e).into())
                        }
                    }
                }
                Err(e) => {
                    log_debug(&format!("❌ Failed to parse API configuration: {}", e));
                    Err(format!("Failed to parse API configuration: {}", e).into())
                }
            }
        } else {
            log_debug("❌ No gRPC client available, returning error");
            Err("No gRPC client connection available".into())
        }
    }
    
    fn parse_api_configuration(&self, message: &Value) -> Result<ModelsApiConfiguration, String> {
        log_debug(&format!("Parsing API configuration from JSON: {}", message));
        
        // 创建基础配置
        let mut api_config = ModelsApiConfiguration::default();
        
        // 解析常见字段
        if let Some(api_key) = message.get("apiKey").and_then(|v| v.as_str()) {
            api_config.api_key = Some(api_key.to_string());
        }
        
        // 解析 planModeApiProvider
        if let Some(plan_provider) = message.get("planModeApiProvider").and_then(|v| v.as_str()) {
            api_config.plan_mode_api_provider = Some(self.parse_api_provider(plan_provider)? as i32);
        }
        
        // 解析 actModeApiProvider
        if let Some(act_provider) = message.get("actModeApiProvider").and_then(|v| v.as_str()) {
            api_config.act_mode_api_provider = Some(self.parse_api_provider(act_provider)? as i32);
        }
        
        // 解析其他可能的字段
        if let Some(model_id) = message.get("planModeApiModelId").and_then(|v| v.as_str()) {
            api_config.plan_mode_api_model_id = Some(model_id.to_string());
        }
        
        if let Some(model_id) = message.get("actModeApiModelId").and_then(|v| v.as_str()) {
            api_config.act_mode_api_model_id = Some(model_id.to_string());
        }
        
        // 解析更多配置字段...
        if let Some(openrouter_key) = message.get("openRouterApiKey").and_then(|v| v.as_str()) {
            api_config.open_router_api_key = Some(openrouter_key.to_string());
        }
        
        if let Some(anthropic_key) = message.get("anthropicApiKey").and_then(|v| v.as_str()) {
            api_config.api_key = Some(anthropic_key.to_string());
        }
        
        if let Some(deepseek_key) = message.get("deepSeekApiKey").and_then(|v| v.as_str()) {
            api_config.deep_seek_api_key = Some(deepseek_key.to_string());
        }
        
        log_debug(&format!("✅ Successfully parsed API configuration with {} fields set", 
            self.count_set_fields(&api_config)));
            
        Ok(api_config)
    }
    
    fn parse_api_provider(&self, provider: &str) -> Result<ApiProvider, String> {
        match provider.to_uppercase().as_str() {
            "ANTHROPIC" => Ok(ApiProvider::Anthropic),
            "OPENROUTER" => Ok(ApiProvider::Openrouter),
            "BEDROCK" => Ok(ApiProvider::Bedrock),
            "VERTEX" => Ok(ApiProvider::Vertex),
            "OPENAI" => Ok(ApiProvider::Openai),
            "OLLAMA" => Ok(ApiProvider::Ollama),
            "LMSTUDIO" => Ok(ApiProvider::Lmstudio),
            "GEMINI" => Ok(ApiProvider::Gemini),
            "OPENAI_NATIVE" => Ok(ApiProvider::OpenaiNative),
            "REQUESTY" => Ok(ApiProvider::Requesty),
            "TOGETHER" => Ok(ApiProvider::Together),
            "DEEPSEEK" => Ok(ApiProvider::Deepseek),
            "QWEN" => Ok(ApiProvider::Qwen),
            "DOUBAO" => Ok(ApiProvider::Doubao),
            "MISTRAL" => Ok(ApiProvider::Mistral),
            "VSCODE_LM" => Ok(ApiProvider::VscodeLm),
            "CLINE" => Ok(ApiProvider::Cline),
            "LITELLM" => Ok(ApiProvider::Litellm),
            "NEBIUS" => Ok(ApiProvider::Nebius),
            "FIREWORKS" => Ok(ApiProvider::Fireworks),
            "ASKSAGE" => Ok(ApiProvider::Asksage),
            "XAI" => Ok(ApiProvider::Xai),
            "SAMBANOVA" => Ok(ApiProvider::Sambanova),
            "CEREBRAS" => Ok(ApiProvider::Cerebras),
            "GROQ" => Ok(ApiProvider::Groq),
            "SAPAICORE" => Ok(ApiProvider::Sapaicore),
            "CLAUDE_CODE" => Ok(ApiProvider::ClaudeCode),
            "MOONSHOT" => Ok(ApiProvider::Moonshot),
            "HUGGINGFACE" => Ok(ApiProvider::Huggingface),
            "HUAWEI_CLOUD_MAAS" => Ok(ApiProvider::HuaweiCloudMaas),
            "BASETEN" => Ok(ApiProvider::Baseten),
            "ZAI" => Ok(ApiProvider::Zai),
            "VERCEL_AI_GATEWAY" => Ok(ApiProvider::VercelAiGateway),
            _ => Err(format!("Unknown API provider: {}", provider))
        }
    }
    
    fn count_set_fields(&self, config: &ModelsApiConfiguration) -> usize {
        let mut count = 0;
        if config.api_key.is_some() { count += 1; }
        if config.plan_mode_api_provider.is_some() { count += 1; }
        if config.act_mode_api_provider.is_some() { count += 1; }
        if config.plan_mode_api_model_id.is_some() { count += 1; }
        if config.act_mode_api_model_id.is_some() { count += 1; }
        if config.open_router_api_key.is_some() { count += 1; }
        if config.deep_seek_api_key.is_some() { count += 1; }
        count
    }
    
    async fn handle_other_methods(&self, method: &str) -> GrpcResult<Value> {
        // 为其他方法返回相应的默认值
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
