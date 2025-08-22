#[cfg(test)]
mod performance_tests {
    use crate::grpc_client::{
        connection::{ClineGrpcClient, ConnectionConfig},
        types::{CacheConfig, LruCache},
        utils::PerformanceStats,
    };
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use serde_json::json;
    use tokio::test;
    use futures::future::join_all;

    #[test]
    async fn test_cache_performance() {
        let config = CacheConfig {
            max_entries: 1000,
            ttl: Duration::from_secs(60),
            enable_compression: false,
        };
        let mut cache = LruCache::new(config);
        
        let start_time = Instant::now();
        
        // 测试大量插入操作
        for i in 0..1000 {
            let key = format!("key_{}", i);
            let value = json!({
                "id": i,
                "data": format!("test_data_{}", i),
                "timestamp": i as u64,
                "metadata": {
                    "type": "test",
                    "category": "performance"
                }
            });
            cache.put(key, value);
        }
        
        let insert_duration = start_time.elapsed();
        println!("[PERF] 1000 cache inserts took: {:?}", insert_duration);
        
        // 测试大量查询操作
        let query_start = Instant::now();
        let mut hit_count = 0;
        
        for i in 0..1000 {
            let key = format!("key_{}", i);
            if cache.get(&key).is_some() {
                hit_count += 1;
            }
        }
        
        let query_duration = query_start.elapsed();
        println!("[PERF] 1000 cache queries took: {:?}", query_duration);
        
        assert_eq!(hit_count, 1000);
        assert!(insert_duration < Duration::from_millis(100)); // 插入应该很快
        assert!(query_duration < Duration::from_millis(50));   // 查询应该更快
        
        let stats = cache.get_stats();
        assert_eq!(stats["entries"], 1000);
        assert_eq!(stats["hits"], 1000);
    }

    #[test]
    async fn test_cache_memory_efficiency() {
        let config = CacheConfig {
            max_entries: 100,
            ttl: Duration::from_secs(60),
            enable_compression: false,
        };
        let mut cache = LruCache::new(config);
        
        // 插入超过最大容量的数据，测试LRU驱逐
        for i in 0..200 {
            let key = format!("key_{}", i);
            let value = json!({"large_data": "x".repeat(1000), "id": i});
            cache.put(key, value);
        }
        
        let stats = cache.get_stats();
        assert_eq!(stats["entries"], 100); // 应该只保留100个条目
        
        // 验证最新的条目仍在缓存中
        assert!(cache.get("key_199").is_some());
        assert!(cache.get("key_150").is_some());
        
        // 验证最旧的条目已被驱逐
        assert!(cache.get("key_0").is_none());
        assert!(cache.get("key_50").is_none());
    }

    #[test]
    async fn test_concurrent_cache_access() {
        let cache = Arc::new(Mutex::new(LruCache::new(CacheConfig {
            max_entries: 1000,
            ttl: Duration::from_secs(60),
            enable_compression: false,
        })));
        
        let start_time = Instant::now();
        
        // 创建多个并发任务
        let tasks: Vec<_> = (0..10).map(|thread_id| {
            let cache = cache.clone();
            tokio::spawn(async move {
                let mut local_hits = 0;
                
                for i in 0..100 {
                    let key = format!("thread_{}_{}", thread_id, i);
                    let value = json!({"thread": thread_id, "index": i});
                    
                    // 插入
                    {
                        let mut cache = cache.lock().unwrap();
                        cache.put(key.clone(), value);
                    }
                    
                    // 立即查询
                    {
                        let mut cache = cache.lock().unwrap();
                        if cache.get(&key).is_some() {
                            local_hits += 1;
                        }
                    }
                }
                
                local_hits
            })
        }).collect();
        
        // 等待所有任务完成
        let results = join_all(tasks).await;
        let total_hits: usize = results.into_iter().map(|r| r.unwrap()).sum();
        
        let duration = start_time.elapsed();
        println!("[PERF] Concurrent cache operations took: {:?}", duration);
        
        assert_eq!(total_hits, 1000); // 每个线程100次命中
        assert!(duration < Duration::from_millis(500)); // 并发操作应该合理快速
        
        let final_stats = cache.lock().unwrap().get_stats();
        assert_eq!(final_stats["entries"], 1000);
    }

    #[test]
    async fn test_performance_stats_efficiency() {
        let mut stats = PerformanceStats::default();
        let start_time = Instant::now();
        
        // 记录大量请求
        for i in 0..10000 {
            let duration = Duration::from_micros(i % 1000); // 变化的持续时间
            let success = i % 10 != 0; // 10%错误率
            stats.record_request(duration, success);
        }
        
        let recording_duration = start_time.elapsed();
        println!("[PERF] Recording 10000 stats took: {:?}", recording_duration);
        
        // 测试统计计算
        let calc_start = Instant::now();
        let json_stats = stats.to_json();
        let calc_duration = calc_start.elapsed();
        
        println!("[PERF] Stats calculation took: {:?}", calc_duration);
        
        assert_eq!(stats.request_count, 10000);
        assert_eq!(stats.error_count, 1000);
        assert_eq!(stats.get_error_rate(), 0.1);
        
        assert!(recording_duration < Duration::from_millis(50)); // 记录应该很快
        assert!(calc_duration < Duration::from_millis(10));      // 计算应该更快
        
        // 验证JSON输出
        assert_eq!(json_stats["request_count"], 10000);
        assert_eq!(json_stats["error_count"], 1000);
        assert_eq!(json_stats["error_rate"], 0.1);
    }

    #[test]
    async fn test_cache_ttl_performance() {
        let config = CacheConfig {
            max_entries: 1000,
            ttl: Duration::from_millis(100), // 短TTL
            enable_compression: false,
        };
        let mut cache = LruCache::new(config);
        
        // 插入数据
        for i in 0..100 {
            let key = format!("ttl_key_{}", i);
            let value = json!({"data": i});
            cache.put(key, value);
        }
        
        // 验证数据存在
        assert_eq!(cache.get_stats()["entries"], 100);
        
        // 等待过期
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // 测试清理性能
        let cleanup_start = Instant::now();
        let removed = cache.cleanup_expired();
        let cleanup_duration = cleanup_start.elapsed();
        
        println!("[PERF] Cleanup of {} expired entries took: {:?}", removed, cleanup_duration);
        
        assert_eq!(removed, 100);
        assert_eq!(cache.get_stats()["entries"], 0);
        assert!(cleanup_duration < Duration::from_millis(10)); // 清理应该很快
    }

    #[test]
    async fn test_client_statistics_performance() {
        let config = ConnectionConfig {
            enable_performance_monitoring: true,
            max_concurrent_requests: 100,
            ..Default::default()
        };
        let client = ClineGrpcClient::with_config(config);
        
        // 测试统计信息获取的性能
        let start_time = Instant::now();
        
        for _ in 0..1000 {
            let _connection_info = client.get_connection_info();
            let _performance_stats = client.get_performance_stats();
            let _cache_stats = client.get_cache_stats();
            let _full_stats = client.get_full_stats();
        }
        
        let duration = start_time.elapsed();
        println!("[PERF] 1000 stats retrievals took: {:?}", duration);
        
        assert!(duration < Duration::from_millis(100)); // 统计检索应该很快
    }

    #[test]
    async fn test_large_json_caching() {
        let config = CacheConfig {
            max_entries: 10,
            ttl: Duration::from_secs(60),
            enable_compression: false,
        };
        let mut cache = LruCache::new(config);
        
        // 创建大型JSON对象
        let large_value = json!({
            "data": "x".repeat(10000), // 10KB字符串
            "array": (0..1000).collect::<Vec<i32>>(), // 大数组
            "nested": {
                "level1": {
                    "level2": {
                        "level3": (0..100).map(|i| format!("item_{}", i)).collect::<Vec<String>>()
                    }
                }
            }
        });
        
        let start_time = Instant::now();
        
        // 测试大对象的缓存性能
        for i in 0..10 {
            let key = format!("large_key_{}", i);
            cache.put(key, large_value.clone());
        }
        
        let insert_duration = start_time.elapsed();
        println!("[PERF] 10 large object inserts took: {:?}", insert_duration);
        
        // 测试大对象的查询性能
        let query_start = Instant::now();
        
        for i in 0..10 {
            let key = format!("large_key_{}", i);
            let _result = cache.get(&key);
        }
        
        let query_duration = query_start.elapsed();
        println!("[PERF] 10 large object queries took: {:?}", query_duration);
        
        assert!(insert_duration < Duration::from_millis(50));
        assert!(query_duration < Duration::from_millis(20));
    }

    #[test]
    async fn test_memory_usage_pattern() {
        let config = CacheConfig {
            max_entries: 1000,
            ttl: Duration::from_secs(60),
            enable_compression: false,
        };
        let mut cache = LruCache::new(config);
        
        // 模拟真实使用模式：不同大小的对象
        let patterns = vec![
            ("small", json!({"type": "small", "size": 1})),
            ("medium", json!({"type": "medium", "data": "x".repeat(100)})),
            ("large", json!({"type": "large", "data": "x".repeat(1000), "array": (0..100).collect::<Vec<i32>>()})),
        ];
        
        let start_time = Instant::now();
        
        // 模拟随机访问模式
        for i in 0..1000 {
            let pattern_idx = i % patterns.len();
            let (pattern_type, ref pattern_data) = patterns[pattern_idx];
            
            let key = format!("{}_{}", pattern_type, i);
            cache.put(key.clone(), pattern_data.clone());
            
            // 偶尔查询之前的数据
            if i > 0 && i % 10 == 0 {
                let old_key = format!("{}_{}", pattern_type, i - 10);
                cache.get(&old_key);
            }
        }
        
        let duration = start_time.elapsed();
        println!("[PERF] Mixed pattern operations took: {:?}", duration);
        
        let stats = cache.get_stats();
        println!("[PERF] Final cache stats: {}", stats);
        
        assert!(duration < Duration::from_millis(200));
        assert!(stats["hit_rate"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn test_performance_monitoring_overhead() {
        let mut stats_enabled = PerformanceStats::default();
        let mut stats_disabled = PerformanceStats::default();
        
        let iterations = 100000;
        
        // 测试启用性能监控的开销
        let start_enabled = Instant::now();
        for i in 0..iterations {
            stats_enabled.record_request(Duration::from_nanos(i % 1000000), i % 10 != 0);
        }
        let enabled_duration = start_enabled.elapsed();
        
        // 测试禁用性能监控的开销（虽然这里我们还是调用，但在实际实现中可以跳过）
        let start_disabled = Instant::now();
        for i in 0..iterations {
            // 在实际实现中，如果禁用了监控，这些调用会被跳过
            if false { // 模拟禁用状态
                stats_disabled.record_request(Duration::from_nanos(i % 1000000), i % 10 != 0);
            }
        }
        let disabled_duration = start_disabled.elapsed();
        
        println!("[PERF] {} stats operations with monitoring: {:?}", iterations, enabled_duration);
        println!("[PERF] {} stats operations without monitoring: {:?}", iterations, disabled_duration);
        
        // 性能监控的开销应该是可接受的
        assert!(enabled_duration < Duration::from_millis(100));
        
        // 验证统计正确性
        assert_eq!(stats_enabled.request_count, iterations);
        assert!(stats_enabled.error_count > 0);
    }
}