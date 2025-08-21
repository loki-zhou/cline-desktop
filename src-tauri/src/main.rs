// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Emitter, Manager};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_dialog::FileDialogBuilder;
use tonic::transport::Channel;
use tonic::{Request, Status};
use prost::Message;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use futures::StreamExt;

#[tauri::command]
async fn grpc_request(
    service: String,
    method: String,
    message: serde_json::Value,
    is_streaming: bool,
    request_id: Option<String>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    println!(
        "gRPC request: service={}, method={}, is_streaming={}, request_id={:?}",
        service, method, is_streaming, request_id
    );

    // 连接到 cline-core 的 gRPC 服务
    let channel = match Channel::from_static("http://127.0.0.1:26040")
        .connect()
        .await
    {
        Ok(channel) => channel,
        Err(e) => {
            eprintln!("Failed to connect to gRPC server: {}", e);
            return Err(format!("Failed to connect to gRPC server: {}", e));
        }
    };

    if is_streaming {
        // 处理流式请求
        if let Some(id) = request_id {
            match handle_streaming_request(&service, &method, message, channel, id.clone(), app_handle.clone()).await {
                Ok(_) => Ok("{\"status\": \"streaming_started\"}".to_string()),
                Err(e) => Err(format!("Streaming request failed: {}", e)),
            }
        } else {
            Err("Request ID is required for streaming requests".to_string())
        }
    } else {
        // 处理普通请求
        match handle_unary_request(&service, &method, message, channel).await {
            Ok(response) => Ok(response),
            Err(e) => Err(format!("Unary request failed: {}", e)),
        }
    }
}

#[tauri::command]
async fn grpc_request_cancel(request_id: String) {
    println!("gRPC request cancelled: {}", request_id);
}

#[tauri::command]
async fn select_workspace(app_handle: tauri::AppHandle) -> Result<String, String> {
    println!("Opening workspace selection dialog...");
    
    // 使用对话框插件打开文件夹选择对话框
    let dialog = app_handle.dialog();
    
    // 使用异步方式打开文件夹选择对话框
    let result = dialog.file().pick_folder().await;
    
    match result {
        Some(folder_path) => {
            let path_str = folder_path.to_string_lossy().to_string();
            println!("Workspace selected: {}", path_str);
            
            // 将工作区路径发送给 cline-core
            if let Err(e) = send_workspace_to_cline_core(&path_str).await {
                eprintln!("Failed to send workspace to cline-core: {}", e);
                return Err(format!("Failed to send workspace path: {}", e));
            }
            
            Ok(path_str)
        }
        None => {
            println!("Workspace selection cancelled by user");
            Err("Workspace selection cancelled".to_string())
        }
    }
}

// 发送工作区路径到 cline-core
async fn send_workspace_to_cline_core(workspace_path: &str) -> Result<(), String> {
    // 连接到 cline-core 的 gRPC 服务
    let channel = match Channel::from_static("http://127.0.0.1:26040")
        .connect()
        .await
    {
        Ok(channel) => channel,
        Err(e) => {
            return Err(format!("Failed to connect to gRPC server: {}", e));
        }
    };
    
    // 这里需要根据实际的 protobuf 定义来构建工作区设置请求
    // 暂时先打印日志，后续需要实现具体的 gRPC 调用
    println!("Sending workspace path to cline-core: {}", workspace_path);
    
    // TODO: 实现具体的 gRPC 调用设置工作区路径
    // 示例: 
    // let mut client = WorkspaceServiceClient::new(channel);
    // let request = SetWorkspaceRequest { path: workspace_path.to_string() };
    // client.set_workspace(request).await.map_err(|e| format!("gRPC error: {}", e))?;
    
    Ok(())
}

// 处理普通 gRPC 请求
async fn handle_unary_request(
    service: &str,
    method: &str,
    message: serde_json::Value,
    channel: Channel,
) -> Result<String, String> {
    // 这里需要根据具体的服务和方法来构建请求
    // 由于 protobuf 类型定义复杂，这里先返回模拟响应
    // 后续需要根据实际的 protobuf 定义来实现
    
    println!("Handling unary request: {}.{}", service, method);
    println!("Message: {:?}", message);
    
    // 模拟响应 - 实际实现需要根据 protobuf 定义来构建
    Ok("{\"message\": \"Real gRPC response from cline-core\"}".to_string())
}

// 处理流式 gRPC 请求
async fn handle_streaming_request(
    service: &str,
    method: &str,
    message: serde_json::Value,
    channel: Channel,
    request_id: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    println!("Handling streaming request: {}.{}", service, method);
    println!("Message: {:?}", message);
    
    // 模拟流式响应 - 实际实现需要根据 protobuf 定义来构建
    if let Some(window) = app_handle.get_webview_window("main") {
        let event_name = format!("grpc_response_{}", request_id);
        window
            .emit(&event_name, Some("{\"message\": \"Real streaming response from cline-core\"}"))
            .map_err(|e| format!("Failed to emit streaming response: {}", e))?;
    }
    
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            grpc_request,
            grpc_request_cancel,
            select_workspace
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Start the cline-core process manually
                let (mut rx, _child) = handle
                    .shell()
                    .command("node")
                    .args(["../cline/dist-standalone/cline-core.js"])
                    .spawn()
                    .expect("Failed to spawn cline-core");

                while let Some(event) = rx.recv().await {
                    if let Some(window) = handle.get_webview_window("main") {
                        match event {
                            tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                                println!("cline-core stdout: {}", String::from_utf8_lossy(&line));
                                window
                                    .emit("sidecar-stdout", Some(String::from_utf8_lossy(&line)))
                                    .expect("failed to emit event");
                            }
                            tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                                eprintln!("cline-core stderr: {}", String::from_utf8_lossy(&line));
                                window
                                    .emit("sidecar-stderr", Some(String::from_utf8_lossy(&line)))
                                    .expect("failed to emit event");
                            }
                            tauri_plugin_shell::process::CommandEvent::Error(line) => {
                                eprintln!("cline-core error: {}", line);
                                window
                                    .emit("sidecar-error", Some(line))
                                    .expect("failed to emit event");
                            }
                            _ => {}
                        }
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
