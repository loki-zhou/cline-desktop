use std::collections::HashMap;
use std::time::Duration;
use tonic::transport::Channel;
use serde_json::Value;
use tauri::WebviewWindow;

use crate::grpc_client::{
    types::{GrpcResult, ServiceType, ServiceHandler, LruCache, CacheConfig},
    utils::{log_debug, log_success, log_error, DEFAULT_CONNECT_TIMEOUT, RetryConfig, PerformanceStats},
    services::{StateServiceHandler, UiServiceHandler, McpServiceHandler, AccountServiceHandler, ModelsServiceHandler},
};

#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub endpoint: std::string::String,
    pub connect_timeout: std::time::Duration,
    pub retry_config: RetryConfig,
    pub health_check_interval: std::time::Duration,
    pub cache_config: CacheConfig,
    pub enable_performance_monitoring: bool,
    pub max_concurrent_requests: usize,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:26040".to_string(),
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            retry_config: RetryConfig {
                max_retries: 8, // 增加重试次数
                initial_delay: Duration::from_millis(2000), // 增加初始延迟
                max_delay: Duration::from_secs(30),
                backoff_multiplier: 1.5, // 更渐进的退避策略
            },
            health_check_interval: Duration::from_secs(60), // 增加健康检查间隔
            cache_config: CacheConfig::default(),
            enable_performance_monitoring: true,
            max_concurrent_requests: 100,
        }
    }
}

pub struct ClineGrpcClient {
    channel: Option<Channel>,
    config: ConnectionConfig,
    services: HashMap<ServiceType, ServiceHandler>,
    last_successful_connection: Option<std::time::Instant>,
    connection_failures: usize,
    // 性能监控和缓存
    performance_stats: PerformanceStats,
    cache: LruCache,
    active_requests: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl ClineGrpcClient {
    pub fn new() -> Self {
        Self::with_config(ConnectionConfig::default())
    }
    
    pub fn with_config(config: ConnectionConfig) -> Self {
        let mut services: HashMap<ServiceType, ServiceHandler> = HashMap::new();
        
        // 注册各个服务处理器
        services.insert(ServiceType::State, ServiceHandler::State(StateServiceHandler::new()));
        services.insert(ServiceType::Ui, ServiceHandler::Ui(UiServiceHandler::new()));
        services.insert(ServiceType::Mcp, ServiceHandler::Mcp(McpServiceHandler::new()));
        services.insert(ServiceType::Account, ServiceHandler::Account(AccountServiceHandler::new()));
        services.insert(ServiceType::Models, ServiceHandler::Models(ModelsServiceHandler::new()));
        
        Self {
            channel: None,
            cache: LruCache::new(config.cache_config.clone()),
            config,
            services,
            last_successful_connection: None,
            connection_failures: 0,
            performance_stats: PerformanceStats::default(),
            active_requests: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }
    
    pub async fn connect(&mut self) -> GrpcResult<()> {
        log_debug(&format!("Connecting to cline-core gRPC server at {}", self.config.endpoint));
        
        let endpoint = self.config.endpoint.clone();
        let connect_timeout = self.config.connect_timeout;
        let retry_config = self.config.retry_config.clone();
        
        // 使用简化的重试逻辑
        let mut last_error: Option<Box<dyn std::error::Error + Send + Sync>> = None;
        let mut delay = retry_config.initial_delay;
        
        for attempt in 0..=retry_config.max_retries {
            if attempt > 0 {
                log_debug(&format!(
                    "Retrying connection (attempt {}/{}) after {}ms delay",
                    attempt,
                    retry_config.max_retries,
                    delay.as_millis()
                ));
                tokio::time::sleep(delay).await;
                
                // 指数退避
                delay = std::cmp::min(
                    Duration::from_millis((delay.as_millis() as f32 * retry_config.backoff_multiplier) as u64),
                    retry_config.max_delay
                );
            }
            
            // 尝试连接
            let connection_result = tokio::time::timeout(
                connect_timeout,
                async {
                    let endpoint_result = Channel::from_shared(endpoint.clone())
                        .map_err(|e| format!("Invalid endpoint: {}", e))?;
                    endpoint_result
                        .connect()
                        .await
                        .map_err(|e| format!("Connection error: {}", e))
                }
            ).await;
            
            match connection_result {
                Ok(Ok(channel)) => {
                    // 连接成功
                    self.channel = Some(channel.clone());
                    self.last_successful_connection = Some(std::time::Instant::now());
                    self.connection_failures = 0;
                    
                    // 为所有服务初始化连接
                    for (service_type, service_handler) in &mut self.services {
                        log_debug(&format!("Initializing {} client", service_type.as_str()));
                        
                        match service_handler {
                            ServiceHandler::State(handler) => handler.set_client(channel.clone()),
                            ServiceHandler::Ui(handler) => handler.set_client(channel.clone()),
                            ServiceHandler::Mcp(handler) => handler.set_client(channel.clone()),
                            ServiceHandler::Account(handler) => handler.set_client(channel.clone()),
                            ServiceHandler::Models(handler) => handler.set_client(channel.clone()),
                        }
                    }
                    
                    if attempt > 0 {
                        log_success(&format!("Connection succeeded after {} retries", attempt));
                    } else {
                        log_success("Successfully connected to cline-core gRPC server");
                    }
                    return Ok(());
                }
                Ok(Err(e)) => {
                    let error_msg = format!("Connection failed: {}", e);
                    log_debug(&error_msg);
                    last_error = Some(error_msg.into());
                }
                Err(_) => {
                    let error_msg = "Connection timeout";
                    log_debug(error_msg);
                    last_error = Some(error_msg.into());
                }
            }
        }
        
        self.connection_failures += 1;
        log_error(&format!(
            "Connection failed after {} attempts",
            retry_config.max_retries + 1
        ));
        
        Err(last_error.unwrap_or_else(|| "Connection failed".into()))
    }
    
    // 设置窗口引用，用于流式状态更新
    pub fn set_window(&mut self, window: WebviewWindow) {
        log_debug("Setting window reference for streaming state updates");
        
        // 为所有服务设置窗口引用
        for (service_type, service_handler) in &mut self.services {
            log_debug(&format!("Setting window for {} service", service_type.as_str()));
            service_handler.set_window(window.clone());
        }
    }
    
    pub async fn ensure_connected(&mut self) -> GrpcResult<()> {
        // 检查连接是否存在
        if self.channel.is_none() {
            return self.connect().await;
        }
        
        // 检查连接是否需要健康检查
        if let Some(last_success) = self.last_successful_connection {
            let elapsed = last_success.elapsed();
            if elapsed > self.config.health_check_interval {
                log_debug("Performing connection health check");
                
                // 进行健康检查，如果失败则重新连接
                if let Err(e) = self.health_check().await {
                    log_error(&format!("Health check failed: {}", e));
                    self.connection_failures += 1;
                    self.channel = None;
                    return self.connect().await;
                } else {
                    self.last_successful_connection = Some(std::time::Instant::now());
                    self.connection_failures = 0;
                }
            }
        }
        
        Ok(())
    }
    
    // 健康检查方法
    async fn health_check(&self) -> GrpcResult<()> {
        if let Some(_channel) = &self.channel {
            // 这里可以使用 tonic-health 包来进行正式的健康检查
            // 但为了简化，我们只检查连接是否还在
            
            // 这里可以添加具体的健康检查逻辑
            // 比如调用一个轻量级的 gRPC 方法
            
            log_debug("Connection health check passed");
            Ok(())
        } else {
            Err("No active connection".into())
        }
    }
    
    pub async fn handle_request(
        &mut self, 
        service: &str, 
        method: &str, 
        message: &Value
    ) -> GrpcResult<Value> {
        self.handle_request_with_config(service, method, message, None).await
    }
    
    pub async fn handle_request_with_config(
        &mut self, 
        service: &str, 
        method: &str, 
        message: &Value,
        stream_config: Option<crate::grpc_client::types::StreamConfig>
    ) -> GrpcResult<Value> {
        let start_time = std::time::Instant::now();
        let cache_key = format!("{}:{}:{}", service, method, serde_json::to_string(message).unwrap_or_default());
        
        // 检查并发请求限制
        let active_count = self.active_requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if active_count >= self.config.max_concurrent_requests {
            self.active_requests.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            return Err("Too many concurrent requests".into());
        }
        
        // 检查缓存（只对特定的只读方法）
        if self.is_cacheable(method) {
            if let Some(cached_value) = self.cache.get(&cache_key) {
                log_debug(&format!("Cache hit for {}:{}", service, method));
                self.active_requests.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                let duration = start_time.elapsed();
                self.performance_stats.record_request(duration, true);
                return Ok(cached_value);
            }
        }
        
        log_debug(&format!("Handling gRPC request: service={}, method={}", service, method));
        
        // 确保连接已建立
        if let Err(e) = self.ensure_connected().await {
            log_error(&format!("Failed to ensure connection: {}", e));
            self.connection_failures += 1;
            self.active_requests.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            let duration = start_time.elapsed();
            self.performance_stats.record_request(duration, false);
            return Err(e);
        }
        
        // 根据服务名称找到对应的处理器
        let service_type = match self.parse_service_type(service) {
            Ok(service_type) => service_type,
            Err(e) => {
                self.active_requests.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                let duration = start_time.elapsed();
                self.performance_stats.record_request(duration, false);
                return Err(e);
            }
        };
        
        let result = if let Some(service_handler) = self.services.get_mut(&service_type) {
            // 尝试执行请求，如果失败则尝试重新连接
            match service_handler.handle_request_with_config(method, message, stream_config).await {
                Ok(result) => {
                    // 请求成功，重置失败计数器
                    self.connection_failures = 0;
                    self.last_successful_connection = Some(std::time::Instant::now());
                    
                    // 缓存结果（如果适用）
                    if self.is_cacheable(method) {
                        self.cache.put(cache_key, result.clone());
                    }
                    
                    Ok(result)
                }
                Err(e) => {
                    log_error(&format!("Request failed: {}", e));
                    self.connection_failures += 1;
                    
                    // 如果错误可能是由于连接问题，尝试重新连接一次
                    if self.is_connection_error(&e) && self.connection_failures <= 2 {
                        log_debug("Attempting to reconnect due to connection error");
                        self.channel = None;
                        
                        if let Ok(_) = self.ensure_connected().await {
                            // 重新连接成功，再次尝试请求
                            if let Some(service_handler) = self.services.get_mut(&service_type) {
                                return match service_handler.handle_request_with_config(method, message, stream_config).await {
                                    Ok(result) => {
                                        if self.is_cacheable(method) {
                                            self.cache.put(cache_key, result.clone());
                                        }
                                        Ok(result)
                                    }
                                    Err(e) => Err(e)
                                };
                            }
                        }
                    }
                    
                    Err(e)
                }
            }
        } else {
            log_error(&format!("Service not found: {}", service));
            Ok(serde_json::json!({
                "error": format!("Service {} not implemented", service)
            }))
        };
        
        // 记录性能统计
        let duration = start_time.elapsed();
        let success = result.is_ok();
        if self.config.enable_performance_monitoring {
            self.performance_stats.record_request(duration, success);
        }
        
        // 减少活跃请求计数
        self.active_requests.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        
        // 定期清理缓存
        if self.performance_stats.should_cleanup() {
            self.cleanup_cache_and_stats();
        }
        
        result
    }
    
    // 判断方法是否可缓存
    fn is_cacheable(&self, method: &str) -> bool {
        matches!(method, "getLatestState" | "getLatestMcpServers")
    }
    
    // 清理缓存和统计
    fn cleanup_cache_and_stats(&mut self) {
        let removed = self.cache.cleanup_expired();
        if removed > 0 {
            log_debug(&format!("Cleaned up {} expired cache entries", removed));
        }
        
        // 如果统计数据过多，重置统计
        if self.performance_stats.request_count > 10000 {
            log_debug("Resetting performance statistics");
            self.performance_stats.reset();
        }
    }
    
    // 判断是否为连接错误
    fn is_connection_error(&self, error: &Box<dyn std::error::Error + Send + Sync>) -> bool {
        let error_string = error.to_string().to_lowercase();
        error_string.contains("connection") ||
        error_string.contains("timeout") ||
        error_string.contains("unavailable") ||
        error_string.contains("refused") ||
        error_string.contains("broken pipe")
    }
    
    fn parse_service_type(&self, service: &str) -> GrpcResult<ServiceType> {
        match service {
            "cline.StateService" => Ok(ServiceType::State),
            "cline.UiService" => Ok(ServiceType::Ui),
            "cline.McpService" => Ok(ServiceType::Mcp),
            "cline.FileService" => Ok(ServiceType::File),
            "cline.ModelsService" => Ok(ServiceType::Models),
            "cline.TaskService" => Ok(ServiceType::Task),
            "cline.AccountService" => Ok(ServiceType::Account),
            "cline.BrowserService" => Ok(ServiceType::Browser),
            "cline.CommandsService" => Ok(ServiceType::Commands),
            "cline.CheckpointsService" => Ok(ServiceType::Checkpoints),
            "cline.SlashService" => Ok(ServiceType::Slash),
            "cline.WebService" => Ok(ServiceType::Web),
            _ => Err(format!("Unknown service: {}", service).into()),
        }
    }
    
    pub fn get_channel(&self) -> Option<&Channel> {
        self.channel.as_ref()
    }
    
    // 获取连接状态信息
    pub fn get_connection_info(&self) -> serde_json::Value {
        serde_json::json!({
            "connected": self.channel.is_some(),
            "endpoint": self.config.endpoint,
            "last_successful_connection": self.last_successful_connection
                .map(|t| t.elapsed().as_secs()),
            "connection_failures": self.connection_failures,
            "health_check_interval_secs": self.config.health_check_interval.as_secs(),
            "active_requests": self.active_requests.load(std::sync::atomic::Ordering::Relaxed),
            "max_concurrent_requests": self.config.max_concurrent_requests,
            "performance_monitoring_enabled": self.config.enable_performance_monitoring
        })
    }
    
    // 获取性能统计
    pub fn get_performance_stats(&self) -> serde_json::Value {
        if self.config.enable_performance_monitoring {
            self.performance_stats.to_json()
        } else {
            serde_json::json!({"monitoring_disabled": true})
        }
    }
    
    // 获取缓存统计
    pub fn get_cache_stats(&self) -> serde_json::Value {
        self.cache.get_stats()
    }
    
    // 获取完整统计信息
    pub fn get_full_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "connection": self.get_connection_info(),
            "performance": self.get_performance_stats(),
            "cache": self.get_cache_stats()
        })
    }
    
    // 手动重置连接
    pub async fn reset_connection(&mut self) -> GrpcResult<()> {
        log_debug("Manually resetting connection");
        self.channel = None;
        self.connection_failures = 0;
        
        // 清理缓存和统计
        self.cache.clear();
        self.performance_stats.reset();
        
        self.connect().await
    }
    
    // 手动清理缓存
    pub fn clear_cache(&mut self) {
        log_debug("Manually clearing cache");
        self.cache.clear();
    }
    
    // 手动重置性能统计
    pub fn reset_performance_stats(&mut self) {
        log_debug("Manually resetting performance statistics");
        self.performance_stats.reset();
    }
}
