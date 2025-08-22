#[cfg(test)]
mod utils_tests {
    use crate::grpc_client::utils::{
        RetryConfig, PerformanceStats, with_retry, with_timeout,
        DEFAULT_CONNECT_TIMEOUT, DEFAULT_REQUEST_TIMEOUT, DEFAULT_MAX_RETRIES,
        log_debug, log_success, log_error,
    };
    use std::time::{Duration, Instant};
    use std::sync::{Arc, Mutex};
    use tokio::test;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        
        assert_eq!(config.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(config.backoff_multiplier, 2.0);
        assert!(config.initial_delay >= Duration::from_millis(500));
        assert!(config.max_delay >= Duration::from_secs(10));
    }

    #[test]
    fn test_retry_config_new() {
        let config = RetryConfig::new(5);
        
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_retry_config_with_delays() {
        let config = RetryConfig::with_delays(
            3,
            Duration::from_millis(100),
            Duration::from_secs(5),
        );
        
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay, Duration::from_millis(100));
        assert_eq!(config.max_delay, Duration::from_secs(5));
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    async fn test_with_timeout_success() {
        let future = async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok::<_, tonic::Status>("success".to_string())
        };
        
        let result = with_timeout(
            future,
            Duration::from_millis(100),
            "test_operation"
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[test]
    async fn test_with_timeout_failure() {
        let future = async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok::<_, tonic::Status>("should_not_complete".to_string())
        };
        
        let result = with_timeout(
            future,
            Duration::from_millis(50),
            "test_operation"
        ).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timeout"));
    }

    #[test]
    async fn test_with_retry_success_first_attempt() {
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();
        
        let operation = move || {
            let counter = counter_clone.clone();
            async move {
                let mut count = counter.lock().unwrap();
                *count += 1;
                Ok::<_, String>("success")
            }
        };
        
        let config = RetryConfig::new(3);
        let result = with_retry(operation, config, "test_op").await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        
        let final_count = *counter.lock().unwrap();
        assert_eq!(final_count, 1); // 只尝试了一次
    }

    #[test]
    async fn test_with_retry_success_after_retries() {
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();
        
        let operation = move || {
            let counter = counter_clone.clone();
            async move {
                let mut count = counter.lock().unwrap();
                *count += 1;
                
                if *count < 3 {
                    Err("temporary failure".to_string())
                } else {
                    Ok("success after retries")
                }
            }
        };
        
        let config = RetryConfig::new(3);
        let result = with_retry(operation, config, "test_op").await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success after retries");
        
        let final_count = *counter.lock().unwrap();
        assert_eq!(final_count, 3); // 尝试了3次
    }

    #[test]
    async fn test_with_retry_failure_after_max_retries() {
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();
        
        let operation = move || {
            let counter = counter_clone.clone();
            async move {
                let mut count = counter.lock().unwrap();
                *count += 1;
                Err::<&str, _>("persistent failure".to_string())
            }
        };
        
        let config = RetryConfig::new(2);
        let result = with_retry(operation, config, "test_op").await;
        
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "persistent failure");
        
        let final_count = *counter.lock().unwrap();
        assert_eq!(final_count, 3); // 初试 + 2次重试 = 3次
    }

    #[test]
    async fn test_exponential_backoff() {
        let config = RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
        };
        
        let start_time = Instant::now();
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();
        
        let operation = move || {
            let counter = counter_clone.clone();
            async move {
                let mut count = counter.lock().unwrap();
                *count += 1;
                Err::<&str, _>("always fail".to_string())
            }
        };
        
        let _result = with_retry(operation, config, "backoff_test").await;
        
        let elapsed = start_time.elapsed();
        
        // 验证总时间包含了退避延迟
        // 预期延迟: 10ms + 20ms + 40ms = 70ms (加上一些容差)
        assert!(elapsed >= Duration::from_millis(60));
        assert!(elapsed <= Duration::from_millis(200)); // 给予一些容差
    }

    #[test]
    fn test_performance_stats_default() {
        let stats = PerformanceStats::default();
        
        assert_eq!(stats.request_count, 0);
        assert_eq!(stats.total_duration, Duration::ZERO);
        assert_eq!(stats.average_duration, Duration::ZERO);
        assert_eq!(stats.error_count, 0);
        assert_eq!(stats.get_error_rate(), 0.0);
    }

    #[test]
    fn test_performance_stats_record_requests() {
        let mut stats = PerformanceStats::default();
        
        // 记录成功请求
        stats.record_request(Duration::from_millis(100), true);
        stats.record_request(Duration::from_millis(200), true);
        
        assert_eq!(stats.request_count, 2);
        assert_eq!(stats.error_count, 0);
        assert_eq!(stats.get_error_rate(), 0.0);
        assert_eq!(stats.max_duration, Duration::from_millis(200));
        assert_eq!(stats.min_duration, Duration::from_millis(100));
        assert_eq!(stats.average_duration, Duration::from_millis(150));
        
        // 记录失败请求
        stats.record_request(Duration::from_millis(50), false);
        
        assert_eq!(stats.request_count, 3);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.get_error_rate(), 1.0 / 3.0);
        assert_eq!(stats.min_duration, Duration::from_millis(50));
    }

    #[test]
    fn test_performance_stats_json_serialization() {
        let mut stats = PerformanceStats::default();
        
        stats.record_request(Duration::from_millis(100), true);
        stats.record_request(Duration::from_millis(200), false);
        
        let json = stats.to_json();
        
        assert_eq!(json["request_count"], 2);
        assert_eq!(json["total_duration_ms"], 300);
        assert_eq!(json["average_duration_ms"], 150);
        assert_eq!(json["max_duration_ms"], 200);
        assert_eq!(json["min_duration_ms"], 100);
        assert_eq!(json["error_count"], 1);
        assert_eq!(json["error_rate"], 0.5);
        assert!(json["uptime_seconds"].is_number());
    }

    #[test]
    fn test_performance_stats_reset() {
        let mut stats = PerformanceStats::default();
        
        // 记录一些数据
        stats.record_request(Duration::from_millis(100), true);
        stats.record_request(Duration::from_millis(200), false);
        
        assert_eq!(stats.request_count, 2);
        assert_eq!(stats.error_count, 1);
        
        // 重置
        stats.reset();
        
        assert_eq!(stats.request_count, 0);
        assert_eq!(stats.error_count, 0);
        assert_eq!(stats.total_duration, Duration::ZERO);
        assert_eq!(stats.average_duration, Duration::ZERO);
        assert_eq!(stats.get_error_rate(), 0.0);
    }

    #[test]
    async fn test_performance_stats_should_cleanup() {
        let stats = PerformanceStats::default();
        
        // 新创建的统计不应该需要清理
        assert!(!stats.should_cleanup());
        
        // 这里我们无法轻易测试时间过期的情况，因为 MEMORY_CLEANUP_INTERVAL 是5分钟
        // 在真实场景中，可能需要使用依赖注入来使时间间隔可测试
    }

    #[test]
    fn test_logging_functions() {
        // 测试日志函数不会panic
        log_debug("Test debug message");
        log_success("Test success message");
        log_error("Test error message");
        
        // 这些函数主要是输出到console，我们主要测试它们不会崩溃
        assert!(true);
    }

    #[test]
    fn test_constants() {
        // 验证常量定义合理
        assert!(DEFAULT_CONNECT_TIMEOUT >= Duration::from_millis(1000));
        assert!(DEFAULT_REQUEST_TIMEOUT >= Duration::from_millis(5000));
        assert!(DEFAULT_MAX_RETRIES >= 1);
        
        // 验证性能监控常量
        assert!(crate::grpc_client::utils::PERFORMANCE_LOG_THRESHOLD >= Duration::from_millis(100));
        assert!(crate::grpc_client::utils::MEMORY_CLEANUP_INTERVAL >= Duration::from_secs(60));
        assert!(crate::grpc_client::utils::MAX_RETRY_DELAY >= Duration::from_secs(10));
    }

    #[test]
    async fn test_with_timeout_with_grpc_error() {
        let future = async {
            Err::<String, _>(tonic::Status::internal("Internal server error"))
        };
        
        let result = with_timeout(
            future,
            Duration::from_millis(100),
            "grpc_error_test"
        ).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("failed"));
    }

    #[test]
    fn test_performance_stats_edge_cases() {
        let mut stats = PerformanceStats::default();
        
        // 测试最小值的初始化
        assert_eq!(stats.min_duration, Duration::MAX);
        
        // 记录一个请求后，min_duration应该更新
        stats.record_request(Duration::from_millis(50), true);
        assert_eq!(stats.min_duration, Duration::from_millis(50));
        
        // JSON序列化应该正确处理初始最小值
        stats.reset();
        let json = stats.to_json();
        assert_eq!(json["min_duration_ms"], 0);
    }

    #[test]
    async fn test_retry_with_zero_retries() {
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();
        
        let operation = move || {
            let counter = counter_clone.clone();
            async move {
                let mut count = counter.lock().unwrap();
                *count += 1;
                Err::<&str, _>("always fail".to_string())
            }
        };
        
        let config = RetryConfig::new(0); // 不重试
        let result = with_retry(operation, config, "no_retry_test").await;
        
        assert!(result.is_err());
        
        let final_count = *counter.lock().unwrap();
        assert_eq!(final_count, 1); // 只尝试了一次，没有重试
    }
}

// 集成测试：测试工具函数的组合使用
#[cfg(test)]
mod utils_integration_tests {
    use crate::grpc_client::utils::{with_retry_and_timeout, RetryConfig};
    use std::time::Duration;
    use std::sync::{Arc, Mutex};
    use tokio::test;

    #[test]
    async fn test_retry_and_timeout_success() {
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();
        
        let operation = move || {
            let counter = counter_clone.clone();
            async move {
                let mut count = counter.lock().unwrap();
                *count += 1;
                
                if *count < 2 {
                    Err(tonic::transport::Error::from_source(
                        std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused")
                    ))
                } else {
                    Ok("success after retry")
                }
            }
        };
        
        let result = with_retry_and_timeout(
            operation,
            RetryConfig::new(3),
            Duration::from_millis(100),
            "integrated_test"
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success after retry");
        
        let final_count = *counter.lock().unwrap();
        assert_eq!(final_count, 2);
    }

    #[test]
    async fn test_retry_and_timeout_timeout_failure() {
        let operation = move || {
            async move {
                // 模拟一个慢操作
                tokio::time::sleep(Duration::from_millis(200)).await;
                Ok::<&str, tonic::transport::Error>("should not complete")
            }
        };
        
        let result = with_retry_and_timeout(
            operation,
            RetryConfig::new(2),
            Duration::from_millis(50), // 很短的超时
            "timeout_test"
        ).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timeout"));
    }

    #[test]
    async fn test_retry_and_timeout_persistent_failure() {
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();
        
        let operation = move || {
            let counter = counter_clone.clone();
            async move {
                let mut count = counter.lock().unwrap();
                *count += 1;
                
                Err(tonic::transport::Error::from_source(
                    std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Permission denied")
                ))
            }
        };
        
        let result = with_retry_and_timeout(
            operation,
            RetryConfig::new(2),
            Duration::from_millis(100),
            "persistent_failure_test"
        ).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("failed"));
        
        let final_count = *counter.lock().unwrap();
        assert_eq!(final_count, 3); // 初试 + 2次重试
    }
}