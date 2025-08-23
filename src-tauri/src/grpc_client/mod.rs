pub mod connection;
pub mod services;
pub mod types;
pub mod utils;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_utils;
#[cfg(test)]
mod tests_performance;

// 导入生成的 protobuf 代码
pub mod cline {
    tonic::include_proto!("cline");
}

// 重新导出公共接口
pub use connection::ClineGrpcClient;
pub use types::*;

// 使用简单的 Arc 共享客户端，无需锁
use std::sync::Arc;
use tokio::sync::RwLock;

lazy_static::lazy_static! {
    static ref GLOBAL_CLIENT: Arc<RwLock<ClineGrpcClient>> = Arc::new(RwLock::new(ClineGrpcClient::new()));
}

pub async fn get_global_client() -> Arc<RwLock<ClineGrpcClient>> {
    GLOBAL_CLIENT.clone()
}