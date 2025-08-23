use serde_json::Value;
use tonic::transport::Channel;
use crate::grpc_client::{
    types::{GrpcResult, StreamConfig},
    utils::log_debug,
};

#[derive(Debug)]
pub struct AccountServiceHandler {
    client: Option<Channel>,
}

impl AccountServiceHandler {
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
        log_debug(&format!("AccountService method not implemented: {}", method));
        
        // 为不同的方法返回相应的默认值
        match method {
            "subscribeToAuthStatusUpdate" => {
                Ok(serde_json::json!({
                    "isAuthenticated": false,
                    "user": null,
                    "timestamp": chrono::Utc::now().timestamp_millis()
                }))
            }
            "getUserCredits" => {
                Ok(serde_json::json!({
                    "balance": {"currentBalance": 0},
                    "usageTransactions": [],
                    "paymentTransactions": []
                }))
            }
            "getUserOrganizations" => {
                Ok(serde_json::json!({
                    "organizations": []
                }))
            }
            "accountLoginClicked" => {
                Ok(serde_json::json!({
                    "loginUrl": "",
                    "success": false
                }))
            }
            "accountLogoutClicked" => {
                Ok(serde_json::json!({}))
            }
            "authStateChanged" => {
                Ok(serde_json::json!({
                    "isAuthenticated": false,
                    "user": null
                }))
            }
            "getOrganizationCredits" => {
                Ok(serde_json::json!({
                    "balance": {"currentBalance": 0},
                    "usageTransactions": [],
                    "organizationId": ""
                }))
            }
            "setUserOrganization" => {
                Ok(serde_json::json!({}))
            }
            "openrouterAuthClicked" => {
                Ok(serde_json::json!({}))
            }
            _ => {
                log_debug(&format!("Unknown AccountService method: {}", method));
                Ok(serde_json::json!({}))
            }
        }
    }
}