/// # Cline Desktop gRPC 客户端使用示例
/// 
/// 这个文件包含了在实际应用中使用 gRPC 客户端的各种示例。

use crate::grpc_client::{
    connection::{ClineGrpcClient, ConnectionConfig},
    types::{ServiceType, StreamConfig, StreamCallback, CacheConfig},
    utils::RetryConfig,
    get_global_client,
};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

/// 基本用法示例：使用全局客户端发送简单请求
pub async fn basic_usage_example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== 基本用法示例 ===");
    
    // 获取全局客户端实例
    let client = get_global_client().await;
    let mut client = client.lock().await;
    
    // 发送 UI 服务请求
    let response = client.handle_request(
        "cline.UiService",
        "subscribeToPartialMessage",
        &json!({})
    ).await?;
    
    println!("UI服务响应: {}", response);
    
    // 发送状态服务请求
    let state_response = client.handle_request(
        "cline.StateService",
        "getLatestState",
        &json!({})
    ).await?;
    
    println!("状态服务响应: {}", state_response);
    
    Ok(())
}

/// 自定义配置示例：创建具有特定配置的客户端
pub async fn custom_configuration_example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== 自定义配置示例 ===");
    
    // 创建自定义配置
    let config = ConnectionConfig {
        endpoint: "http://127.0.0.1:26040".to_string(),
        connect_timeout: Duration::from_secs(10),
        retry_config: RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        },
        health_check_interval: Duration::from_secs(15),
        cache_config: CacheConfig {
            max_entries: 500,
            ttl: Duration::from_secs(300), // 5分钟TTL
            enable_compression: false,
        },
        enable_performance_monitoring: true,
        max_concurrent_requests: 50,
    };
    
    let mut client = ClineGrpcClient::with_config(config);
    
    // 手动连接
    println!("正在连接到 gRPC 服务器...");
    client.connect().await?;
    
    // 发送请求
    let response = client.handle_request(
        "cline.McpService",
        "getLatestMcpServers",
        &json!({})
    ).await?;
    
    println!("MCP服务响应: {}", response);
    
    // 查看连接信息
    let connection_info = client.get_connection_info();
    println!("连接信息: {}", connection_info);
    
    Ok(())
}

/// 流式处理示例：处理流式响应
pub async fn streaming_example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== 流式处理示例 ===");
    
    let client = get_global_client().await;
    let mut client = client.lock().await;
    
    // 创建流式消息处理回调
    let message_count = Arc::new(std::sync::Mutex::new(0));
    let message_count_clone = message_count.clone();
    
    let callback: StreamCallback = Arc::new(move |message| {
        let mut count = message_count_clone.lock().unwrap();
        *count += 1;
        
        println!("收到流式消息 #{}: {}", *count, message);
        
        // 可以在这里处理特定的消息逻辑
        if let Some(message_type) = message.get("type") {
            match message_type.as_str() {
                Some("assistant") => println!("  -> 这是助手消息"),
                Some("user") => println!("  -> 这是用户消息"),
                _ => println!("  -> 未知消息类型"),
            }
        }
        
        Ok(())
    });
    
    // 配置流式处理
    let stream_config = StreamConfig {
        enable_streaming: true,
        callback: Some(callback),
        max_messages: Some(10), // 最多处理10条消息
    };
    
    // 获取 UI 服务处理器并发送流式请求
    if let Some(crate::grpc_client::types::ServiceHandler::Ui(ui_handler)) = 
        client.services.get_mut(&ServiceType::Ui) {
        
        let response = ui_handler.handle_request_with_config(
            "subscribeToPartialMessage",
            &json!({}),
            Some(stream_config)
        ).await?;
        
        println!("流式处理完成，最终响应: {}", response);
    }
    
    let final_count = *message_count.lock().unwrap();
    println!("总共处理了 {} 条流式消息", final_count);
    
    Ok(())
}

/// 错误处理示例：演示各种错误处理策略
pub async fn error_handling_example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== 错误处理示例 ===");
    
    let client = get_global_client().await;
    let mut client = client.lock().await;
    
    // 1. 处理服务不存在的错误
    match client.handle_request("invalid.Service", "test", &json!({})).await {
        Ok(response) => println!("意外的成功响应: {}", response),
        Err(e) => println!("处理服务不存在错误: {}", e),
    }
    
    // 2. 处理方法不存在的错误
    match client.handle_request("cline.UiService", "nonexistentMethod", &json!({})).await {
        Ok(response) => println!("方法处理响应: {}", response),
        Err(e) => println!("处理方法错误: {}", e),
    }
    
    // 3. 连接重置示例
    println!("尝试重置连接...");
    match client.reset_connection().await {
        Ok(_) => println!("连接重置成功"),
        Err(e) => println!("连接重置失败: {}", e),
    }
    
    // 4. 处理超时场景（使用短超时配置）
    let timeout_config = ConnectionConfig {
        connect_timeout: Duration::from_millis(1), // 极短超时
        ..Default::default()
    };
    
    let mut timeout_client = ClineGrpcClient::with_config(timeout_config);
    match timeout_client.connect().await {
        Ok(_) => println!("意外的连接成功"),
        Err(e) => println!("预期的超时错误: {}", e),
    }
    
    Ok(())
}

/// 性能监控示例：展示如何监控和优化性能
pub async fn performance_monitoring_example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== 性能监控示例 ===");
    
    let client = get_global_client().await;
    let mut client = client.lock().await;
    
    // 发送一些请求来生成性能数据
    for i in 0..5 {
        let response = client.handle_request(
            "cline.UiService",
            "subscribeToPartialMessage",
            &json!({"request_id": i})
        ).await;
        
        match response {
            Ok(_) => println!("请求 {} 成功", i),
            Err(e) => println!("请求 {} 失败: {}", i, e),
        }
    }
    
    // 获取性能统计
    let performance_stats = client.get_performance_stats();
    println!("性能统计:");
    println!("  请求总数: {}", performance_stats["request_count"]);
    println!("  平均响应时间: {}ms", performance_stats["average_duration_ms"]);
    println!("  最大响应时间: {}ms", performance_stats["max_duration_ms"]);
    println!("  错误率: {:.2}%", 
        performance_stats["error_rate"].as_f64().unwrap_or(0.0) * 100.0);
    
    // 获取缓存统计
    let cache_stats = client.get_cache_stats();
    println!("缓存统计:");
    println!("  缓存条目数: {}", cache_stats["entries"]);
    println!("  缓存命中数: {}", cache_stats["hits"]);
    println!("  缓存未命中数: {}", cache_stats["misses"]);
    println!("  缓存命中率: {:.2}%", 
        cache_stats["hit_rate"].as_f64().unwrap_or(0.0) * 100.0);
    
    // 获取连接信息
    let connection_info = client.get_connection_info();
    println!("连接信息:");
    println!("  连接状态: {}", if connection_info["connected"].as_bool().unwrap_or(false) { "已连接" } else { "未连接" });
    println!("  活跃请求数: {}", connection_info["active_requests"]);
    println!("  连接失败次数: {}", connection_info["connection_failures"]);
    
    // 获取完整统计
    let full_stats = client.get_full_stats();
    println!("完整统计信息:");
    println!("{}", serde_json::to_string_pretty(&full_stats)?);
    
    Ok(())
}

/// 缓存使用示例：演示如何有效使用缓存
pub async fn caching_example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== 缓存使用示例 ===");
    
    let client = get_global_client().await;
    let mut client = client.lock().await;
    
    println!("首次请求（会被缓存）:");
    let start_time = std::time::Instant::now();
    let response1 = client.handle_request(
        "cline.StateService",
        "getLatestState", // 这是可缓存的方法
        &json!({})
    ).await;
    let duration1 = start_time.elapsed();
    
    match response1 {
        Ok(_) => println!("  请求成功，耗时: {:?}", duration1),
        Err(e) => println!("  请求失败: {}", e),
    }
    
    println!("第二次相同请求（应该命中缓存）:");
    let start_time = std::time::Instant::now();
    let response2 = client.handle_request(
        "cline.StateService",
        "getLatestState",
        &json!({})
    ).await;
    let duration2 = start_time.elapsed();
    
    match response2 {
        Ok(_) => println!("  请求成功，耗时: {:?}", duration2),
        Err(e) => println!("  请求失败: {}", e),
    }
    
    // 比较响应时间
    if duration2 < duration1 {
        println!("  ✅ 缓存生效！第二次请求更快");
    } else {
        println!("  ❓ 缓存可能未生效或服务器未运行");
    }
    
    // 查看缓存统计
    let cache_stats = client.get_cache_stats();
    println!("缓存统计: {}", cache_stats);
    
    // 手动清理缓存
    println!("清理缓存...");
    client.clear_cache();
    
    let cache_stats_after = client.get_cache_stats();
    println!("清理后缓存统计: {}", cache_stats_after);
    
    Ok(())
}

/// 并发处理示例：演示并发请求处理
pub async fn concurrent_requests_example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== 并发处理示例 ===");
    
    let client = get_global_client().await;
    
    // 创建多个并发任务
    let tasks: Vec<_> = (0..5).map(|i| {
        let client = client.clone();
        tokio::spawn(async move {
            let mut client = client.lock().await;
            let start_time = std::time::Instant::now();
            
            let response = client.handle_request(
                "cline.UiService",
                "subscribeToPartialMessage",
                &json!({"concurrent_id": i})
            ).await;
            
            let duration = start_time.elapsed();
            
            match response {
                Ok(_) => println!("  并发任务 {} 成功，耗时: {:?}", i, duration),
                Err(e) => println!("  并发任务 {} 失败: {}", i, e),
            }
            
            (i, response.is_ok(), duration)
        })
    }).collect();
    
    // 等待所有任务完成
    let results = futures::future::join_all(tasks).await;
    
    let mut success_count = 0;
    let mut total_duration = Duration::ZERO;
    
    for result in results {
        if let Ok((id, success, duration)) = result {
            if success {
                success_count += 1;
            }
            total_duration += duration;
            println!("任务 {} 完成", id);
        }
    }
    
    println!("并发处理结果:");
    println!("  成功任务数: {}/5", success_count);
    println!("  平均耗时: {:?}", total_duration / 5);
    
    // 检查客户端状态
    let client = client.lock().await;
    let connection_info = client.get_connection_info();
    println!("当前活跃请求数: {}", connection_info["active_requests"]);
    
    Ok(())
}

/// 配置调优示例：针对不同场景的配置优化
pub async fn configuration_tuning_examples() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== 配置调优示例 ===");
    
    // 1. 高并发场景配置
    println!("1. 高并发场景配置:");
    let high_concurrency_config = ConnectionConfig {
        endpoint: "http://127.0.0.1:26040".to_string(),
        max_concurrent_requests: 100, // 高并发限制
        cache_config: CacheConfig {
            max_entries: 2000,          // 大缓存
            ttl: Duration::from_secs(600), // 长TTL
            enable_compression: false,
        },
        retry_config: RetryConfig::new(3), // 中等重试
        enable_performance_monitoring: true,
        ..Default::default()
    };
    
    let _high_concurrency_client = ClineGrpcClient::with_config(high_concurrency_config);
    println!("  ✅ 已创建高并发客户端");
    
    // 2. 低延迟场景配置
    println!("2. 低延迟场景配置:");
    let low_latency_config = ConnectionConfig {
        endpoint: "http://127.0.0.1:26040".to_string(),
        connect_timeout: Duration::from_millis(500), // 短超时
        retry_config: RetryConfig::new(1),           // 少重试
        health_check_interval: Duration::from_secs(5), // 频繁健康检查
        cache_config: CacheConfig {
            max_entries: 100,                        // 小缓存
            ttl: Duration::from_secs(30),           // 短TTL
            enable_compression: false,
        },
        enable_performance_monitoring: false,        // 禁用监控减少开销
        max_concurrent_requests: 10,
    };
    
    let _low_latency_client = ClineGrpcClient::with_config(low_latency_config);
    println!("  ✅ 已创建低延迟客户端");
    
    // 3. 可靠性优先场景配置
    println!("3. 可靠性优先场景配置:");
    let reliability_config = ConnectionConfig {
        endpoint: "http://127.0.0.1:26040".to_string(),
        connect_timeout: Duration::from_secs(30),   // 长超时
        retry_config: RetryConfig {
            max_retries: 10,                        // 多次重试
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(60),     // 长最大延迟
            backoff_multiplier: 1.5,                // 温和的退避
        },
        health_check_interval: Duration::from_secs(10), // 定期健康检查
        cache_config: CacheConfig {
            max_entries: 500,
            ttl: Duration::from_secs(1800),         // 30分钟TTL
            enable_compression: false,
        },
        enable_performance_monitoring: true,
        max_concurrent_requests: 20,
    };
    
    let _reliability_client = ClineGrpcClient::with_config(reliability_config);
    println!("  ✅ 已创建高可靠性客户端");
    
    Ok(())
}

/// 完整的应用示例：结合多个功能的实际应用场景
pub async fn complete_application_example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== 完整应用示例 ===");
    
    // 创建应用级配置
    let app_config = ConnectionConfig {
        endpoint: "http://127.0.0.1:26040".to_string(),
        connect_timeout: Duration::from_secs(10),
        retry_config: RetryConfig::new(3),
        health_check_interval: Duration::from_secs(30),
        cache_config: CacheConfig {
            max_entries: 1000,
            ttl: Duration::from_secs(300),
            enable_compression: false,
        },
        enable_performance_monitoring: true,
        max_concurrent_requests: 50,
    };
    
    let mut client = ClineGrpcClient::with_config(app_config);
    
    println!("1. 初始化应用...");
    
    // 尝试连接
    match client.connect().await {
        Ok(_) => println!("  ✅ 已连接到 cline-core 服务器"),
        Err(e) => {
            println!("  ❌ 连接失败: {}", e);
            println!("  ℹ️  这是正常的，因为可能没有运行 cline-core 服务器");
        }
    }
    
    println!("2. 获取应用状态...");
    
    // 获取当前状态（可缓存）
    let state_result = client.handle_request(
        "cline.StateService",
        "getLatestState",
        &json!({})
    ).await;
    
    match state_result {
        Ok(state) => {
            println!("  ✅ 获取状态成功");
            if let Some(state_data) = state.get("data") {
                println!("     状态数据: {}", state_data);
            }
        }
        Err(e) => println!("  ❌ 获取状态失败: {}", e),
    }
    
    println!("3. 设置流式消息监听...");
    
    // 设置流式消息处理
    let message_handler: StreamCallback = Arc::new(|message| {
        println!("  📨 收到新消息: {}", message);
        
        // 在实际应用中，这里可能会：
        // - 更新UI状态
        // - 触发特定的业务逻辑
        // - 记录日志
        // - 通知其他组件
        
        Ok(())
    });
    
    let stream_config = StreamConfig {
        enable_streaming: true,
        callback: Some(message_handler),
        max_messages: Some(5),
    };
    
    // 开始监听流式消息
    if let Some(crate::grpc_client::types::ServiceHandler::Ui(ui_handler)) = 
        client.services.get_mut(&ServiceType::Ui) {
        
        let stream_result = ui_handler.handle_request_with_config(
            "subscribeToPartialMessage",
            &json!({}),
            Some(stream_config)
        ).await;
        
        match stream_result {
            Ok(_) => println!("  ✅ 流式消息监听已设置"),
            Err(e) => println!("  ❌ 流式消息监听失败: {}", e),
        }
    }
    
    println!("4. 执行业务操作...");
    
    // 模拟一些业务操作
    let operations = vec![
        ("cline.McpService", "getLatestMcpServers"),
        ("cline.UiService", "subscribeToPartialMessage"),
        ("cline.StateService", "getLatestState"),
    ];
    
    for (service, method) in operations {
        let result = client.handle_request(
            service,
            method,
            &json!({"timestamp": chrono::Utc::now().timestamp()})
        ).await;
        
        match result {
            Ok(_) => println!("  ✅ {} -> {} 成功", service, method),
            Err(e) => println!("  ❌ {} -> {} 失败: {}", service, method, e),
        }
        
        // 短暂延迟模拟实际应用场景
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    println!("5. 生成应用报告...");
    
    // 生成综合报告
    let full_stats = client.get_full_stats();
    
    println!("\n📊 应用统计报告:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    if let Some(connection) = full_stats.get("connection") {
        println!("🔗 连接状态:");
        println!("   连接状态: {}", if connection["connected"].as_bool().unwrap_or(false) { "✅ 已连接" } else { "❌ 未连接" });
        println!("   失败次数: {}", connection["connection_failures"]);
        println!("   活跃请求: {}", connection["active_requests"]);
    }
    
    if let Some(performance) = full_stats.get("performance") {
        println!("⚡ 性能统计:");
        println!("   请求总数: {}", performance["request_count"]);
        println!("   平均响应: {}ms", performance["average_duration_ms"]);
        println!("   错误率: {:.1}%", performance["error_rate"].as_f64().unwrap_or(0.0) * 100.0);
    }
    
    if let Some(cache) = full_stats.get("cache") {
        println!("💾 缓存统计:");
        println!("   缓存条目: {}", cache["entries"]);
        println!("   命中率: {:.1}%", cache["hit_rate"].as_f64().unwrap_or(0.0) * 100.0);
    }
    
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    println!("6. 清理和关闭...");
    
    // 清理缓存
    client.clear_cache();
    
    // 重置性能统计
    client.reset_performance_stats();
    
    println!("  ✅ 应用示例完成");
    
    Ok(())
}

/// 运行所有示例的主函数
pub async fn run_all_examples() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 开始运行 Cline Desktop gRPC 客户端示例");
    println!("════════════════════════════════════════════════════════════");
    
    // 运行各个示例
    let examples = vec![
        ("基本用法", basic_usage_example as fn() -> _),
        ("自定义配置", custom_configuration_example as fn() -> _),
        ("流式处理", streaming_example as fn() -> _),
        ("错误处理", error_handling_example as fn() -> _),
        ("性能监控", performance_monitoring_example as fn() -> _),
        ("缓存使用", caching_example as fn() -> _),
        ("并发处理", concurrent_requests_example as fn() -> _),
        ("配置调优", configuration_tuning_examples as fn() -> _),
        ("完整应用", complete_application_example as fn() -> _),
    ];
    
    for (name, example_fn) in examples {
        println!("\n📋 运行示例: {}", name);
        println!("────────────────────────────────────────────────────────────");
        
        match example_fn().await {
            Ok(_) => println!("✅ {} 示例完成\n", name),
            Err(e) => println!("❌ {} 示例失败: {}\n", name, e),
        }
        
        // 在示例之间添加短暂延迟
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    println!("🎉 所有示例运行完成！");
    println!("════════════════════════════════════════════════════════════");
    
    Ok(())
}

// 注：在实际使用中，您需要在 main.rs 或其他入口文件中调用这些示例：
//
// ```rust
// use crate::grpc_client::examples::run_all_examples;
//
// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//     run_all_examples().await
// }
// ```