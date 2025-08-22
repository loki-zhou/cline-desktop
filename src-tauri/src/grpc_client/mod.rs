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

// 全局客户端实例
use tokio::sync::Mutex;
use std::sync::Arc;

lazy_static::lazy_static! {
    static ref GLOBAL_CLIENT: Arc<Mutex<ClineGrpcClient>> = Arc::new(Mutex::new(ClineGrpcClient::new()));
}

pub async fn get_global_client() -> Arc<Mutex<ClineGrpcClient>> {
    GLOBAL_CLIENT.clone()
}