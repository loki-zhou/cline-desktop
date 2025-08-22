#[cfg(test)]
mod tests {
    use crate::grpc_client::{
        connection::{ClineGrpcClient, ConnectionConfig},
        types::{ServiceType, CacheConfig, LruCache},
        utils::{RetryConfig, PerformanceStats},
    };
    use std::time::Duration;
    use serde_json::json;
    use tokio::test;

    // 创建测试用的配置
    fn create_test_config() -> ConnectionConfig {
        ConnectionConfig {
            endpoint: "http://127.0.0.1:26040".to_string(),
            connect_timeout: Duration::from_millis(500),
            retry_config: RetryConfig::new(2),
            health_check_interval: Duration::from_secs(10),
            cache_config: CacheConfig {
                max_entries: 10,
                ttl: Duration::from_secs(30),
                enable_compression: false,
            },
            enable_performance_monitoring: true,
            max_concurrent_requests: 5,
        }
    }

    #[test]
    async fn test_client_creation() {
        let client = ClineGrpcClient::new();
        let connection_info = client.get_connection_info();
        
        assert_eq!(connection_info["connected"], false);
        assert_eq!(connection_info["endpoint"], "http://127.0.0.1:26040");
        assert_eq!(connection_info["connection_failures"], 0);
    }

    #[test]
    async fn test_client_with_custom_config() {
        let config = create_test_config();
        let client = ClineGrpcClient::with_config(config.clone());
        let connection_info = client.get_connection_info();
        
        assert_eq!(connection_info["endpoint"], config.endpoint);
        assert_eq!(connection_info["max_concurrent_requests"], config.max_concurrent_requests);
    }

    #[test]
    async fn test_service_type_parsing() {
        let client = ClineGrpcClient::new();
        
        // 测试有效的服务类型
        assert!(client.parse_service_type("cline.StateService").is_ok());
        assert!(client.parse_service_type("cline.UiService").is_ok());
        assert!(client.parse_service_type("cline.McpService").is_ok());
        
        // 测试无效的服务类型
        assert!(client.parse_service_type("invalid.Service").is_err());
        assert!(client.parse_service_type("").is_err());
    }

    #[test]
    async fn test_cache_functionality() {
        let config = CacheConfig {
            max_entries: 3,
            ttl: Duration::from_millis(100),
            enable_compression: false,
        };
        let mut cache = LruCache::new(config);
        
        // 测试基本的缓存操作
        cache.put("key1".to_string(), json!({"value": 1}));
        cache.put("key2".to_string(), json!({"value": 2}));
        
        assert!(cache.get("key1").is_some());
        assert!(cache.get("key2").is_some());
        assert!(cache.get("nonexistent").is_none());
        
        // 测试缓存统计
        let stats = cache.get_stats();
        assert_eq!(stats["entries"], 2);
        assert_eq!(stats["hits"], 2);
        assert_eq!(stats["misses"], 1);
    }

    #[test]
    async fn test_cache_ttl_expiration() {
        let config = CacheConfig {
            max_entries: 10,
            ttl: Duration::from_millis(50), // 很短的TTL
            enable_compression: false,
        };
        let mut cache = LruCache::new(config);
        
        cache.put("test_key".to_string(), json!({"data": "test"}));
        
        // 立即访问应该成功
        assert!(cache.get("test_key").is_some());
        
        // 等待过期
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // 现在应该过期了
        assert!(cache.get("test_key").is_none());
    }

    #[test]
    async fn test_cache_eviction() {
        let config = CacheConfig {
            max_entries: 2, // 只允许2个条目
            ttl: Duration::from_secs(60),
            enable_compression: false,
        };
        let mut cache = LruCache::new(config);
        
        cache.put("key1".to_string(), json!(1));
        cache.put("key2".to_string(), json!(2));
        
        // 访问 key1 使其成为最近使用的
        cache.get("key1");
        
        // 添加第三个条目，应该驱逐 key2
        cache.put("key3".to_string(), json!(3));
        
        assert!(cache.get("key1").is_some());
        assert!(cache.get("key2").is_none()); // 应该被驱逐
        assert!(cache.get("key3").is_some());
    }

    #[test]
    fn test_performance_stats() {
        let mut stats = PerformanceStats::default();
        
        // 记录一些请求
        stats.record_request(Duration::from_millis(100), true);
        stats.record_request(Duration::from_millis(200), true);
        stats.record_request(Duration::from_millis(50), false);
        
        assert_eq!(stats.request_count, 3);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.get_error_rate(), 1.0 / 3.0);
        assert_eq!(stats.max_duration, Duration::from_millis(200));
        assert_eq!(stats.min_duration, Duration::from_millis(50));
        
        // 测试JSON序列化
        let json_stats = stats.to_json();
        assert_eq!(json_stats["request_count"], 3);
        assert_eq!(json_stats["error_count"], 1);
    }

    #[test]
    fn test_retry_config() {
        let config = RetryConfig::new(5);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.backoff_multiplier, 2.0);
        
        let custom_config = RetryConfig::with_delays(
            3,
            Duration::from_millis(100),
            Duration::from_secs(10),
        );
        assert_eq!(custom_config.max_retries, 3);
        assert_eq!(custom_config.initial_delay, Duration::from_millis(100));
        assert_eq!(custom_config.max_delay, Duration::from_secs(10));
    }

    #[test]
    async fn test_cacheable_methods() {
        let client = ClineGrpcClient::new();
        
        // 测试可缓存的方法
        assert!(client.is_cacheable("getLatestState"));
        assert!(client.is_cacheable("getLatestMcpServers"));
        
        // 测试不可缓存的方法
        assert!(!client.is_cacheable("subscribeToPartialMessage"));
        assert!(!client.is_cacheable("sendMessage"));
        assert!(!client.is_cacheable("unknown_method"));
    }

    #[test]
    fn test_connection_error_detection() {
        let client = ClineGrpcClient::new();
        
        // 测试连接错误检测
        let connection_error: Box<dyn std::error::Error + Send + Sync> = 
            "connection refused".into();
        assert!(client.is_connection_error(&connection_error));
        
        let timeout_error: Box<dyn std::error::Error + Send + Sync> = 
            "request timeout".into();
        assert!(client.is_connection_error(&timeout_error));
        
        let other_error: Box<dyn std::error::Error + Send + Sync> = 
            "invalid argument".into();
        assert!(!client.is_connection_error(&other_error));
    }

    #[test]
    async fn test_client_statistics() {
        let config = create_test_config();
        let client = ClineGrpcClient::with_config(config);
        
        // 测试获取统计信息
        let connection_info = client.get_connection_info();
        let performance_stats = client.get_performance_stats();
        let cache_stats = client.get_cache_stats();
        let full_stats = client.get_full_stats();
        
        // 验证统计信息结构
        assert!(connection_info.is_object());
        assert!(performance_stats.is_object());
        assert!(cache_stats.is_object());
        assert!(full_stats.is_object());
        
        assert!(full_stats.get("connection").is_some());
        assert!(full_stats.get("performance").is_some());
        assert!(full_stats.get("cache").is_some());
    }

    #[test]
    async fn test_cache_cleanup() {
        let config = CacheConfig {
            max_entries: 10,
            ttl: Duration::from_millis(50),
            enable_compression: false,
        };
        let mut cache = LruCache::new(config);
        
        // 添加一些条目
        cache.put("key1".to_string(), json!(1));
        cache.put("key2".to_string(), json!(2));
        cache.put("key3".to_string(), json!(3));
        
        assert_eq!(cache.get_stats()["entries"], 3);
        
        // 等待过期
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // 执行清理
        let removed = cache.cleanup_expired();
        assert_eq!(removed, 3);
        assert_eq!(cache.get_stats()["entries"], 0);
    }

    #[test]
    fn test_service_type_as_str() {
        assert_eq!(ServiceType::State.as_str(), "cline.StateService");
        assert_eq!(ServiceType::Ui.as_str(), "cline.UiService");
        assert_eq!(ServiceType::Mcp.as_str(), "cline.McpService");
        assert_eq!(ServiceType::File.as_str(), "cline.FileService");
        assert_eq!(ServiceType::Models.as_str(), "cline.ModelsService");
        assert_eq!(ServiceType::Task.as_str(), "cline.TaskService");
        assert_eq!(ServiceType::Account.as_str(), "cline.AccountService");
        assert_eq!(ServiceType::Browser.as_str(), "cline.BrowserService");
        assert_eq!(ServiceType::Commands.as_str(), "cline.CommandsService");
        assert_eq!(ServiceType::Checkpoints.as_str(), "cline.CheckpointsService");
        assert_eq!(ServiceType::Slash.as_str(), "cline.SlashService");
        assert_eq!(ServiceType::Web.as_str(), "cline.WebService");
    }
}

// 集成测试模块
#[cfg(test)]
mod integration_tests {
    use crate::grpc_client::{
        connection::{ClineGrpcClient, ConnectionConfig},
        types::StreamConfig,
    };
    use std::sync::Arc;
    use std::time::Duration;
    use serde_json::json;
    use tokio::test;

    // 模拟的 gRPC 服务器测试（当实际服务器不可用时）
    #[test]
    async fn test_connection_failure_handling() {
        let config = ConnectionConfig {
            endpoint: "http://127.0.0.1:99999".to_string(), // 不存在的端口
            connect_timeout: Duration::from_millis(100),
            retry_config: crate::grpc_client::utils::RetryConfig::new(1),
            ..Default::default()
        };
        
        let mut client = ClineGrpcClient::with_config(config);
        
        // 尝试连接应该失败
        let result = client.connect().await;
        assert!(result.is_err());
        
        // 连接信息应该反映失败状态
        let connection_info = client.get_connection_info();
        assert_eq!(connection_info["connected"], false);
        assert!(connection_info["connection_failures"].as_u64().unwrap() > 0);
    }

    #[test]
    async fn test_request_without_connection() {
        let mut client = ClineGrpcClient::new();
        
        // 不先连接，直接发送请求
        let result = client.handle_request(
            "cline.UiService",
            "subscribeToPartialMessage",
            &json!({})
        ).await;
        
        // 应该尝试自动连接，但由于服务器不存在而失败
        assert!(result.is_err());
    }

    #[test]
    async fn test_concurrent_request_limiting() {
        let config = ConnectionConfig {
            max_concurrent_requests: 2, // 限制为2个并发请求
            ..Default::default()
        };
        let client = Arc::new(tokio::sync::Mutex::new(ClineGrpcClient::with_config(config)));
        
        // 模拟多个并发请求
        let handles: Vec<_> = (0..5).map(|i| {
            let client = client.clone();
            tokio::spawn(async move {
                let mut client = client.lock().await;
                client.handle_request(
                    "cline.UiService",
                    "subscribeToPartialMessage",
                    &json!({"request_id": i})
                ).await
            })
        }).collect();
        
        // 等待所有请求完成
        let results: Vec<_> = futures::future::join_all(handles).await;
        
        // 应该有一些请求由于并发限制而失败
        let errors: Vec<_> = results.into_iter()
            .filter_map(|r| r.ok())
            .filter(|r| r.is_err())
            .collect();
        
        // 至少应该有一些请求被拒绝
        assert!(errors.len() > 0);
    }
}