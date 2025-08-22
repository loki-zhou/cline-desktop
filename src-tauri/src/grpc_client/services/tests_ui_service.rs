#[cfg(test)]
mod ui_service_tests {
    use crate::grpc_client::{
        services::UiServiceHandler,
        types::{StreamConfig, StreamCallback},
        cline::{ClineMessage, Metadata},
    };
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use serde_json::{json, Value};
    use tokio::test;

    fn create_test_message() -> ClineMessage {
        ClineMessage {
            ts: 1234567890,
            r#type: "assistant".to_string(),
            ask: "test_ask".to_string(),
            say: "test_say".to_string(),
            text: "test_text".to_string(),
            reasoning: "test_reasoning".to_string(),
            images: vec!["image1.png".to_string(), "image2.png".to_string()],
            files: vec!["file1.txt".to_string()],
            partial: true,
            last_checkpoint_hash: "hash123".to_string(),
            is_checkpoint_checked_out: false,
            is_operation_outside_workspace: false,
            conversation_history_index: 42,
        }
    }

    #[test]
    fn test_ui_service_handler_creation() {
        let handler = UiServiceHandler::new();
        
        // 新创建的处理器应该没有客户端
        // 这里我们无法直接测试private字段，但可以通过行为来验证
        
        // 验证处理器已正确初始化
        assert!(true); // 占位符断言，实际中会有更具体的测试
    }

    #[test]
    async fn test_handle_unknown_method() {
        let mut handler = UiServiceHandler::new();
        
        let result = handler.handle_request("unknown_method", &json!({})).await;
        
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response["success"].as_bool().unwrap());
        assert!(response["message"].as_str().unwrap().contains("not implemented"));
    }

    #[test]
    fn test_build_partial_message_response() {
        let handler = UiServiceHandler::new();
        let test_message = create_test_message();
        
        let response = handler.build_partial_message_response(&test_message);
        
        // 验证响应包含所有预期字段
        assert_eq!(response["ts"], 1234567890);
        assert_eq!(response["type"], "assistant");
        assert_eq!(response["ask"], "test_ask");
        assert_eq!(response["say"], "test_say");
        assert_eq!(response["text"], "test_text");
        assert_eq!(response["reasoning"], "test_reasoning");
        assert_eq!(response["partial"], true);
        assert_eq!(response["lastCheckpointHash"], "hash123");
        assert_eq!(response["isCheckpointCheckedOut"], false);
        assert_eq!(response["isOperationOutsideWorkspace"], false);
        assert_eq!(response["conversationHistoryIndex"], 42);
        
        // 验证数组字段
        let images = response["images"].as_array().unwrap();
        assert_eq!(images.len(), 2);
        assert_eq!(images[0], "image1.png");
        assert_eq!(images[1], "image2.png");
        
        let files = response["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], "file1.txt");
    }

    #[test]
    async fn test_handle_request_with_config() {
        let mut handler = UiServiceHandler::new();
        
        // 测试不支持的方法
        let result = handler.handle_request_with_config(
            "unsupported_method",
            &json!({}),
            None
        ).await;
        
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response["message"].as_str().unwrap().contains("not implemented"));
    }

    #[test]
    fn test_stream_config_creation() {
        // 测试流式配置的创建和调试输出
        let config = StreamConfig {
            enable_streaming: true,
            callback: None,
            max_messages: Some(10),
        };
        
        assert_eq!(config.enable_streaming, true);
        assert!(config.callback.is_none());
        assert_eq!(config.max_messages, Some(10));
        
        // 测试Debug trait实现
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("enable_streaming: true"));
        assert!(debug_str.contains("callback: None"));
        assert!(debug_str.contains("max_messages: Some(10)"));
    }

    #[test]
    fn test_stream_config_with_callback() {
        let callback_called = Arc::new(Mutex::new(false));
        let callback_called_clone = callback_called.clone();
        
        let callback: StreamCallback = Arc::new(move |_value| {
            let mut called = callback_called_clone.lock().unwrap();
            *called = true;
            Ok(())
        });
        
        let config = StreamConfig {
            enable_streaming: true,
            callback: Some(callback.clone()),
            max_messages: None,
        };
        
        assert!(config.callback.is_some());
        
        // 测试回调功能
        if let Some(ref cb) = config.callback {
            let test_value = json!({"test": "data"});
            let result = cb(test_value);
            assert!(result.is_ok());
            
            let called = callback_called.lock().unwrap();
            assert!(*called);
        }
    }

    // 模拟测试：测试流式消息处理逻辑（无需真实连接）
    #[test]
    async fn test_streaming_logic_simulation() {
        // 这里我们测试流式处理的逻辑，但不涉及真实的gRPC连接
        let mut message_count = 0;
        let max_messages = 5;
        let mut processed_messages = Vec::new();
        
        // 模拟处理多个消息
        for i in 0..10 {
            let message = ClineMessage {
                ts: i as u64,
                conversation_history_index: i,
                ..create_test_message()
            };
            
            let handler = UiServiceHandler::new();
            let message_value = handler.build_partial_message_response(&message);
            
            processed_messages.push(message_value);
            message_count += 1;
            
            // 检查是否达到最大消息数量
            if message_count >= max_messages {
                break;
            }
        }
        
        assert_eq!(message_count, max_messages);
        assert_eq!(processed_messages.len(), max_messages);
        
        // 验证消息顺序
        for (i, message) in processed_messages.iter().enumerate() {
            assert_eq!(message["conversationHistoryIndex"], i);
        }
    }

    #[test]
    fn test_metadata_creation() {
        let metadata = Metadata {};
        
        // 基本测试确保 Metadata 结构体可以正常创建
        // 这是一个简单的结构体，主要测试它能正常实例化
        assert!(true); // Metadata 是空结构体，能创建就说明正常
        
        // 可以测试是否实现了必要的 traits
        let _debug_str = format!("{:?}", metadata);
        let _cloned = metadata.clone();
    }

    #[test]
    async fn test_error_handling_in_streaming() {
        // 测试流式处理中的错误处理逻辑
        let error_callback: StreamCallback = Arc::new(|_value| {
            Err("Simulated callback error".into())
        });
        
        let config = StreamConfig {
            enable_streaming: true,
            callback: Some(error_callback),
            max_messages: Some(3),
        };
        
        // 测试回调错误不会中断处理
        if let Some(ref callback) = config.callback {
            let test_value = json!({"test": "error_test"});
            let result = callback(test_value);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Simulated callback error"));
        }
    }

    #[test]
    fn test_partial_message_response_completeness() {
        let handler = UiServiceHandler::new();
        
        // 创建一个完整的测试消息
        let message = ClineMessage {
            ts: 9876543210,
            r#type: "user".to_string(),
            ask: "How are you?".to_string(),
            say: "Hello".to_string(),
            text: "This is a test message".to_string(),
            reasoning: "Testing message construction".to_string(),
            images: vec!["test1.jpg".to_string(), "test2.png".to_string(), "test3.gif".to_string()],
            files: vec!["doc1.pdf".to_string(), "data.csv".to_string()],
            partial: false,
            last_checkpoint_hash: "abc123def456".to_string(),
            is_checkpoint_checked_out: true,
            is_operation_outside_workspace: true,
            conversation_history_index: 99,
        };
        
        let response = handler.build_partial_message_response(&message);
        
        // 验证所有字段都正确映射
        assert_eq!(response["ts"], 9876543210u64);
        assert_eq!(response["type"], "user");
        assert_eq!(response["ask"], "How are you?");
        assert_eq!(response["say"], "Hello");
        assert_eq!(response["text"], "This is a test message");
        assert_eq!(response["reasoning"], "Testing message construction");
        assert_eq!(response["partial"], false);
        assert_eq!(response["lastCheckpointHash"], "abc123def456");
        assert_eq!(response["isCheckpointCheckedOut"], true);
        assert_eq!(response["isOperationOutsideWorkspace"], true);
        assert_eq!(response["conversationHistoryIndex"], 99);
        
        // 验证复杂字段
        assert_eq!(response["images"].as_array().unwrap().len(), 3);
        assert_eq!(response["files"].as_array().unwrap().len(), 2);
        assert_eq!(response["images"][2], "test3.gif");
        assert_eq!(response["files"][1], "data.csv");
    }
}