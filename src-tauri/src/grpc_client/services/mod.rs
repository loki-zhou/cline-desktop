pub mod state_service;
pub mod ui_service;
pub mod mcp_service;

#[cfg(test)]
mod tests_ui_service;

// 重新导出服务处理器
pub use state_service::StateServiceHandler;
pub use ui_service::UiServiceHandler;
pub use mcp_service::McpServiceHandler;