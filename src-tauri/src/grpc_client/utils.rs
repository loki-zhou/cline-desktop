use std::time::{Duration, Instant};
use tokio::time::{timeout, sleep};
use crate::grpc_client::GrpcResult;

// 公共的超时配置
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

// 重试配置
pub const DEFAULT_MAX_RETRIES: usize = 3;
pub const DEFAULT_RETRY_DELAY: Duration = Duration::from_millis(1000);
pub const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

// 性能监控配置
pub const PERFORMANCE_LOG_THRESHOLD: Duration = Duration::from_millis(1000);
pub const MEMORY_CLEANUP_INTERVAL: Duration = Duration::from_secs(300); // 5分钟

// 重试策略配置
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: usize,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            initial_delay: DEFAULT_RETRY_DELAY,
            max_delay: MAX_RETRY_DELAY,
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    pub fn new(max_retries: usize) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }
    
    pub fn with_delays(max_retries: usize, initial_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_retries,
            initial_delay,
            max_delay,
            backoff_multiplier: 2.0,
        }
    }
}

// 带超时的异步操作包装器
pub async fn with_timeout<F, T>(
    future: F,
    timeout_duration: Duration,
    operation_name: &str,
) -> GrpcResult<T>
where
    F: std::future::Future<Output = Result<T, tonic::Status>>,
{
    timeout(timeout_duration, future)
        .await
        .map_err(|_| format!("{} timeout", operation_name))?
        .map_err(|e| format!("{} failed: {}", operation_name, e).into())
}

// 带重试的异步操作包装器
pub async fn with_retry<F, Fut, T, E>(
    mut operation: F,
    config: RetryConfig,
    operation_name: &str,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display + Clone,
{
    let mut last_error: Option<E> = None;
    let mut delay = config.initial_delay;
    
    for attempt in 0..=config.max_retries {
        if attempt > 0 {
            log_debug(&format!(
                "Retrying {} (attempt {}/{}) after {}ms delay",
                operation_name,
                attempt,
                config.max_retries,
                delay.as_millis()
            ));
            sleep(delay).await;
            
            // 指数退避：每次重试后延迟时间加倍
            delay = std::cmp::min(
                Duration::from_millis((delay.as_millis() as f32 * config.backoff_multiplier) as u64),
                config.max_delay
            );
        }
        
        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    log_success(&format!("{} succeeded after {} retries", operation_name, attempt));
                } else {
                    log_success(&format!("{} succeeded on first attempt", operation_name));
                }
                return Ok(result);
            }
            Err(e) => {
                log_debug(&format!(
                    "{} failed on attempt {}: {}",
                    operation_name,
                    attempt + 1,
                    e
                ));
                last_error = Some(e);
            }
        }
    }
    
    log_error(&format!(
        "{} failed after {} attempts",
        operation_name,
        config.max_retries + 1
    ));
    
    Err(last_error.unwrap())
}

// 带重试和超时的组合包装器
pub async fn with_retry_and_timeout<F, Fut, T>(
    operation: F,
    retry_config: RetryConfig,
    timeout_duration: Duration,
    operation_name: &str,
) -> GrpcResult<T>
where
    F: Fn() -> Fut + Clone,
    Fut: std::future::Future<Output = Result<T, tonic::transport::Error>>,
{
    with_retry(
        || {
            let op = operation.clone();
            async move {
                timeout(timeout_duration, op())
                    .await
                    .map_err(|_| format!("{} timeout", operation_name))
                    .and_then(|result| {
                        result.map_err(|e| format!("{} failed: {}", operation_name, e))
                    })
            }
        },
        retry_config,
        operation_name,
    )
    .await
    .map_err(|e| e.into())
}

// 日志辅助函数
pub fn log_debug(message: &str) {
    println!("[DEBUG] {}", message);
}

pub fn log_success(message: &str) {
    println!("[DEBUG] ✅ {}", message);
}

pub fn log_error(message: &str) {
    println!("[DEBUG] ❌ {}", message);
}

// 性能统计结构
#[derive(Debug, Clone)]
pub struct PerformanceStats {
    pub request_count: u64,
    pub total_duration: Duration,
    pub average_duration: Duration,
    pub max_duration: Duration,
    pub min_duration: Duration,
    pub error_count: u64,
    pub last_reset: Instant,
}

impl Default for PerformanceStats {
    fn default() -> Self {
        Self {
            request_count: 0,
            total_duration: Duration::ZERO,
            average_duration: Duration::ZERO,
            max_duration: Duration::ZERO,
            min_duration: Duration::MAX,
            error_count: 0,
            last_reset: Instant::now(),
        }
    }
}

impl PerformanceStats {
    pub fn record_request(&mut self, duration: Duration, success: bool) {
        self.request_count += 1;
        self.total_duration += duration;
        
        if duration > self.max_duration {
            self.max_duration = duration;
        }
        
        if duration < self.min_duration {
            self.min_duration = duration;
        }
        
        if self.request_count > 0 {
            self.average_duration = self.total_duration / self.request_count as u32;
        }
        
        if !success {
            self.error_count += 1;
        }
        
        // 性能警告
        if duration > PERFORMANCE_LOG_THRESHOLD {
            log_debug(&format!(
                "Slow request detected: {}ms (threshold: {}ms)",
                duration.as_millis(),
                PERFORMANCE_LOG_THRESHOLD.as_millis()
            ));
        }
    }
    
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    
    pub fn get_error_rate(&self) -> f64 {
        if self.request_count == 0 {
            0.0
        } else {
            self.error_count as f64 / self.request_count as f64
        }
    }
    
    pub fn should_cleanup(&self) -> bool {
        self.last_reset.elapsed() > MEMORY_CLEANUP_INTERVAL
    }
    
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "request_count": self.request_count,
            "total_duration_ms": self.total_duration.as_millis(),
            "average_duration_ms": self.average_duration.as_millis(),
            "max_duration_ms": self.max_duration.as_millis(),
            "min_duration_ms": if self.min_duration == Duration::MAX { 0 } else { self.min_duration.as_millis() },
            "error_count": self.error_count,
            "error_rate": self.get_error_rate(),
            "uptime_seconds": self.last_reset.elapsed().as_secs()
        })
    }
}