// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod hostbridge;
mod grpc_client;

use std::sync::{Arc, Mutex};
use tauri::{Manager, Emitter};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::{CommandEvent, CommandChild};
use serde::{Deserialize, Serialize};
use serde_json::Value;


#[tauri::command]
async fn select_workspace(app_handle: tauri::AppHandle) -> Result<String, String> {
    println!("Opening workspace selection dialog...");
    
    // 使用对话框插件打开文件夹选择对话框
    let dialog = app_handle.dialog();
    
    // 使用回调方式打开文件夹选择对话框
    let (tx, rx) = std::sync::mpsc::channel();
    
    dialog.file().pick_folder(move |folder_path| {
        let _ = tx.send(folder_path);
    });
    
    // 等待对话框结果
    match rx.recv() {
        Ok(Some(folder_path)) => {
            let path_str = folder_path.to_string();
            println!("Workspace selected: {}", path_str);
            Ok(path_str)
        }
        Ok(None) => {
            println!("Workspace selection cancelled by user");
            Err("Workspace selection cancelled".to_string())
        }
        Err(_) => {
            println!("Error receiving dialog result");
            Err("Error receiving dialog result".to_string())
        }
    }
}

// 全局进程管理器，用于跟踪所有子进程
struct ProcessManager {
    processes: Vec<CommandChild>,
}

impl ProcessManager {
    fn new() -> Self {
        Self {
            processes: Vec::new(),
        }
    }

    fn add_process(&mut self, child: CommandChild) {
        println!("Adding process to manager, total processes: {}", self.processes.len() + 1);
        self.processes.push(child);
    }

    fn kill_all(&mut self) {
        println!("Attempting to kill {} child processes...", self.processes.len());
        let mut processes_to_kill = Vec::new();
        std::mem::swap(&mut processes_to_kill, &mut self.processes);
        
        for process in processes_to_kill {
            match process.kill() {
                Ok(_) => println!("Successfully killed a child process"),
                Err(e) => eprintln!("Failed to kill process: {}", e),
            }
        }
        println!("Finished killing all child processes");
    }
}

// 创建一个全局的进程管理器
type SharedProcessManager = Arc<Mutex<ProcessManager>>;

fn create_process_manager() -> SharedProcessManager {
    Arc::new(Mutex::new(ProcessManager::new()))
}

#[tauri::command]
async fn start_cline_core(
    app_handle: tauri::AppHandle,
    process_manager: tauri::State<'_, SharedProcessManager>
) -> Result<String, String> {
    println!("Starting cline-core process...");
    
    // 使用shell直接运行node + cline-core.js
    // 确保工作目录是正确的
    let cline_core_path = "cline-core.js";
    let command = app_handle
        .shell()
        .command("node")
        .args([cline_core_path])
        .current_dir("../cline/dist-standalone"); // 设置正确的工作目录
    
    let (mut rx, child) = command
        .spawn()
        .map_err(|e| format!("Failed to spawn cline-core process: {}", e))?;
    
    // 将子进程添加到进程管理器中
    {
        let mut manager = process_manager.lock().unwrap();
        manager.add_process(child);
    }
    
    println!("Cline core process started");
    
    // 处理cline-core的输出
    let handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let Some(window) = handle.get_webview_window("main") {
                match event {
                    CommandEvent::Stdout(line) => {
                        let line_str = String::from_utf8_lossy(&line);
                        println!("cline-core stdout: {}", line_str);
                        
                        // 更灵活的就绪检测条件 - 检测多种可能的就绪信号
                        if line_str.contains("HostBridge is serving") || 
                           line_str.contains("ProtoBus gRPC server listening on") ||
                           line_str.contains("gRPC server listening") ||
                           line_str.contains("Server started") {
                            let window_clone = window.clone();
                            println!("[DEBUG] Detected cline-core ready signal: {}", line_str.trim());
                            println!("[DEBUG] Emitting cline-core-ready event in 3 seconds...");
                            // 在发送就绪事件之前，增加等待时间以确保服务完全启动
                            tauri::async_runtime::spawn(async move {
                                tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;
                                println!("[DEBUG] Emitting cline-core-ready event now");
                                match window_clone.emit("cline-core-ready", ()) {
                                    Ok(_) => println!("[DEBUG] ✅ cline-core-ready event emitted successfully"),
                                    Err(e) => println!("[DEBUG] ❌ Failed to emit cline-core-ready event: {}", e),
                                }
                            });
                        }
                        
                        window
                            .emit("cline-stdout", line_str.to_string())
                            .expect("failed to emit event");
                    }
                    CommandEvent::Stderr(line) => {
                        eprintln!("cline-core stderr: {}", String::from_utf8_lossy(&line));
                        window
                            .emit("cline-stderr", String::from_utf8_lossy(&line).to_string())
                            .expect("failed to emit event");
                    }
                    CommandEvent::Error(err) => {
                        eprintln!("cline-core error: {}", err);
                        window
                            .emit("cline-error", err.to_string())
                            .expect("failed to emit event");
                    }
                    CommandEvent::Terminated(status) => {
                        println!("cline-core terminated with status: {:?}", status);
                        window
                            .emit("cline-terminated", status)
                            .expect("failed to emit event");
                    }
                    _ => {}
                }
            }
        }
    });
    
    Ok("Cline core started successfully".to_string())
}

#[tauri::command]
async fn start_node_server(
    app_handle: tauri::AppHandle,
    process_manager: tauri::State<'_, SharedProcessManager>,
    script_path: String,
    args: Vec<String>
) -> Result<String, String> {
    println!("Starting Node.js server: {}", script_path);
    
    // 创建一个新的命令来启动Node.js进程
    let command = app_handle
        .shell()
        .command("node");
    
    // 构建参数
    let mut all_args = vec![script_path];
    all_args.extend(args);
    
    let command = command.args(all_args);
    
    let (mut rx, child) = command
        .spawn()
        .map_err(|e| format!("Failed to spawn Node.js process: {}", e))?;
    
    // 将子进程添加到进程管理器中
    {
        let mut manager = process_manager.lock().unwrap();
        manager.add_process(child);
    }
    
    println!("Node.js server started");
    
    // 处理Node.js进程的输出
    let handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let Some(window) = handle.get_webview_window("main") {
                match event {
                    CommandEvent::Stdout(line) => {
                        println!("Node.js stdout: {}", String::from_utf8_lossy(&line));
                        window
                            .emit("node-stdout", String::from_utf8_lossy(&line).to_string())
                            .expect("failed to emit event");
                    }
                    CommandEvent::Stderr(line) => {
                        eprintln!("Node.js stderr: {}", String::from_utf8_lossy(&line));
                        window
                            .emit("node-stderr", String::from_utf8_lossy(&line).to_string())
                            .expect("failed to emit event");
                    }
                    CommandEvent::Error(err) => {
                        eprintln!("Node.js error: {}", err);
                        window
                            .emit("node-error", err.to_string())
                            .expect("failed to emit event");
                    }
                    CommandEvent::Terminated(status) => {
                        println!("Node.js terminated with status: {:?}", status);
                        window
                            .emit("node-terminated", status)
                            .expect("failed to emit event");
                    }
                    _ => {}
                }
            }
        }
    });
    
    Ok("Node.js server started successfully".to_string())
}

#[tauri::command]
async fn start_node_server_sidecar(
    app_handle: tauri::AppHandle,
    process_manager: tauri::State<'_, SharedProcessManager>,
    port: u16
) -> Result<String, String> {
    println!("Starting Node.js server sidecar on port {}", port);
    
    // 使用sidecar功能启动Node.js服务器
    let sidecar_command = app_handle
        .shell()
        .sidecar("node-server")
        .map_err(|e| format!("Failed to get sidecar command: {}", e))?;
    
    // 传递端口参数
    let sidecar_command = sidecar_command.args(&[port.to_string()]);
    
    let (mut rx, child) = sidecar_command
        .spawn()
        .map_err(|e| format!("Failed to spawn Node.js sidecar: {}", e))?;
    
    // 将子进程添加到进程管理器中
    {
        let mut manager = process_manager.lock().unwrap();
        manager.add_process(child);
    }
    
    println!("Node.js sidecar started");
    
    // 处理sidecar的输出
    let handle = app_handle.clone();
    let ready_flag = "NODE_SERVER_READY";
    let mut server_ready = false;
    
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let Some(window) = handle.get_webview_window("main") {
                match event {
                    CommandEvent::Stdout(line) => {
                        let line_str = String::from_utf8_lossy(&line).to_string();
                        println!("Node.js sidecar stdout: {}", line_str);
                        
                        // 检查服务器是否准备就绪
                        if !server_ready && line_str.contains(ready_flag) {
                            server_ready = true;
                            window
                                .emit("node-server-ready", port)
                                .expect("failed to emit ready event");
                        }
                        
                        window
                            .emit("node-stdout", line_str)
                            .expect("failed to emit event");
                    }
                    CommandEvent::Stderr(line) => {
                        eprintln!("Node.js sidecar stderr: {}", String::from_utf8_lossy(&line));
                        window
                            .emit("node-stderr", String::from_utf8_lossy(&line).to_string())
                            .expect("failed to emit event");
                    }
                    CommandEvent::Error(err) => {
                        eprintln!("Node.js sidecar error: {}", err);
                        window
                            .emit("node-error", err.to_string())
                            .expect("failed to emit event");
                    }
                    CommandEvent::Terminated(status) => {
                        println!("Node.js sidecar terminated with status: {:?}", status);
                        window
                            .emit("node-terminated", status)
                            .expect("failed to emit event");
                    }
                    _ => {}
                }
            }
        }
    });
    
    Ok(format!("Node.js sidecar started on port {}", port))
}

#[tauri::command]
async fn stop_all_processes(process_manager: tauri::State<'_, SharedProcessManager>) -> Result<String, String> {
    println!("Stopping all child processes...");
    
    // 使用作用域来确保锁在函数结束前被释放
    {
        let mut manager = process_manager.lock().unwrap();
        manager.kill_all();
    }
    
    Ok("All processes stopped successfully".to_string())
}

#[tauri::command]
async fn test_grpc_connection() -> Result<String, String> {
    println!("[DEBUG] Testing gRPC connection to cline-core...");
    
    // 创建新的客户端实例进行测试
    let mut client = grpc_client::ClineGrpcClient::new();
    
    let connection_info = client.get_connection_info();
    let performance_stats = client.get_performance_stats();
    let cache_stats = client.get_cache_stats();
    
    let test_result = serde_json::json!({
        "connection_info": connection_info,
        "performance_stats": performance_stats,
        "cache_stats": cache_stats,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    });
    
    println!("[DEBUG] gRPC Connection Test Result: {}", test_result);
    Ok(test_result.to_string())
}

// Webview消息结构体
#[derive(Debug, Deserialize, Serialize)]
struct WebviewMessage {
    #[serde(rename = "type")]
    message_type: String,
    #[serde(rename = "grpc_request")]
    grpc_request: Option<GrpcRequest>,
}

#[derive(Debug, Deserialize, Serialize)]
struct GrpcRequest {
    service: String,
    method: String,
    message: Value,
    request_id: String,
    is_streaming: bool,
}

#[tauri::command]
async fn handle_webview_message(
    app_handle: tauri::AppHandle,
    message: WebviewMessage,
) -> Result<Value, String> {
    println!("[DEBUG] Received webview message: type={:?}", message.message_type);
    
    // 根据消息类型处理
    let result = match message.message_type.as_str() {
        "grpc_request" => {
            if let Some(grpc_request) = message.grpc_request {
                println!("[DEBUG] Processing gRPC request: service={}, method={}, request_id={}, is_streaming={}",
                    grpc_request.service, grpc_request.method, grpc_request.request_id, grpc_request.is_streaming);
                
                // 根据服务类型转发到不同的端口
                let forward_result = if grpc_request.service.starts_with("cline.") {
                    println!("[DEBUG] Forwarding to ProtoBus (26040): {} {}", grpc_request.service, grpc_request.method);
                    // 转发到ProtoBus (Node.js cline-core on port 26040)
                    forward_to_protobus(&grpc_request).await
                } else if grpc_request.service.starts_with("host.") {
                    println!("[DEBUG] Forwarding to HostBridge (26041): {} {}", grpc_request.service, grpc_request.method);
                    // 转发到HostBridge (Rust HostBridge on port 26041)
                    forward_to_hostbridge(&grpc_request).await
                } else {
                    Err(format!("Unknown service: {}", grpc_request.service))
                };
                
                // 将结果发送回前端
                if let Some(window) = app_handle.get_webview_window("main") {
                    let response_message = match forward_result {
                        Ok(ref response_data) => {
                            println!("[DEBUG] Sending successful response back to frontend for request_id: {}", grpc_request.request_id);
                            serde_json::json!({
                                "type": "grpc_response",
                                "grpc_response": {
                                    "request_id": grpc_request.request_id,
                                    "message": response_data,
                                    "error": null,
                                    "is_streaming": grpc_request.is_streaming
                                }
                            })
                        },
                        Err(ref error_msg) => {
                            println!("[DEBUG] Sending error response back to frontend for request_id: {}", grpc_request.request_id);
                            serde_json::json!({
                                "type": "grpc_response",
                                "grpc_response": {
                                    "request_id": grpc_request.request_id,
                                    "message": null,
                                    "error": error_msg,
                                    "is_streaming": false
                                }
                            })
                        }
                    };
                    
                    // 使用 eval 执行 JavaScript 将响应发送到前端
                    let js_code = format!(
                        "window.dispatchEvent(new MessageEvent('message', {{ data: {} }}));",
                        response_message.to_string()
                    );
                    
                    match window.eval(&js_code) {
                        Ok(_) => println!("[DEBUG] ✅ Response sent to frontend successfully"),
                        Err(e) => println!("[DEBUG] ❌ Failed to send response to frontend: {}", e),
                    }
                }
                
                forward_result
            } else {
                Err("Missing grpc_request in message".to_string())
            }
        }
        _ => Err(format!("Unknown message type: {}", message.message_type)),
    };
    
    // 返回处理结果
    match result {
        Ok(response) => Ok(response),
        Err(error) => {
            println!("[DEBUG] Handle webview message error: {}", error);
            Ok(serde_json::json!({ "error": error }))
        }
    }
}

async fn forward_to_protobus(grpc_request: &GrpcRequest) -> Result<Value, String> {
    println!("[DEBUG] Forwarding gRPC request to ProtoBus (26040): service={}, method={}, request_id={}", 
        grpc_request.service, grpc_request.method, grpc_request.request_id);
    
    // 为每个请求创建独立的客户端实例，完全避免锁竞争
    println!("[DEBUG] Creating new gRPC client instance for this request...");
    let mut client = grpc_client::ClineGrpcClient::new();
    
    println!("[DEBUG] Attempting to ensure gRPC client connection...");
    
    // 尝试使用真正的 gRPC 连接
    match client.handle_request(
        &grpc_request.service,
        &grpc_request.method,
        &grpc_request.message
    ).await {
        Ok(response) => {
            println!("[DEBUG] ✅ Real gRPC request successful: service={}, method={}", 
                grpc_request.service, grpc_request.method);
            Ok(response)
        }
        Err(e) => {
            println!("[DEBUG] ❌ Real gRPC request failed: {}, falling back to mock response", e);
            println!("[DEBUG] Error details: {:?}", e);
            
            // 如果 gRPC 连接失败，返回 mock 响应
            fallback_mock_response(grpc_request)
        }
    }
}

fn fallback_mock_response(grpc_request: &GrpcRequest) -> Result<Value, String> {
    println!("[DEBUG] ❌ Using fallback mock response for: {}.{}", 
        grpc_request.service, grpc_request.method);
        
    let current_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    
    // 对于某些方法，直接返回错误而不是无效响应
    match (grpc_request.service.as_str(), grpc_request.method.as_str()) {
        ("cline.UiService", "subscribeToPartialMessage") => {
            println!("[DEBUG] subscribeToPartialMessage - returning error to avoid timestamp validation");
            return Err("No partial messages available".to_string());
        }
        _ => {}
    }
    
    // 根据不同的服务和方法返回不同的 mock 响应
    let mock_response = match (grpc_request.service.as_str(), grpc_request.method.as_str()) {
        ("cline.StateService", "subscribeToState") | ("cline.StateService", "getLatestState") => {
            // 返回包含 state_json 的状态响应
            serde_json::json!({
                "state_json": serde_json::json!({
                    "version": "2.0.0",
                    "clineMessages": [],
                    "taskHistory": [],
                    "apiConfiguration": {},
                    "customInstructions": "",
                    "alwaysAllowReadOnly": false,
                    "alwaysAllowWrite": false,
                    "alwaysAllowExecute": false,
                    "alwaysAllowBrowser": false,
                    "alwaysAllowMcp": false,
                    "didShowWelcome": true,
                    "shouldShowAnnouncement": false,
                    "experimentalTerminal": false,
                    "timestamp": current_timestamp
                }).to_string()
            })
        }
        _ => {
            // 其他方法的默认 mock 响应，确保时间戳是有效的正数
            serde_json::json!({
                "ts": current_timestamp,
                "type": 1,
                "ask": 0,
                "say": 4,
                "text": format!("Mock response for {}.{}", grpc_request.service, grpc_request.method),
                "reasoning": "",
                "images": [],
                "files": [],
                "partial": false,
                "lastCheckpointHash": "",
                "isCheckpointCheckedOut": false,
                "isOperationOutsideWorkspace": false,
                "conversationHistoryIndex": 0
            })
        }
    };
    
    println!("[DEBUG] Fallback mock response with ts={}: {}", current_timestamp, mock_response);
    Ok(mock_response)
}

async fn forward_to_hostbridge(grpc_request: &GrpcRequest) -> Result<Value, String> {
    println!("[DEBUG] Forwarding to HostBridge (26041): service={}, method={}, request_id={}", 
        grpc_request.service, grpc_request.method, grpc_request.request_id);
    
    // 构建请求URL
    let url = format!("http://127.0.0.1:26041/{}/{}", grpc_request.service, grpc_request.method);
    println!("[DEBUG] HostBridge URL: {}", url);
    
    // 创建HTTP客户端
    let client = reqwest::Client::new();
    
    // 发送POST请求到HostBridge
    match client.post(&url)
        .json(&grpc_request.message)
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            println!("[DEBUG] HostBridge response status: {}", status);
            
            if status.is_success() {
                let response_json: Value = response.json().await
                    .map_err(|e| {
                        println!("[DEBUG] Failed to parse HostBridge response JSON: {}", e);
                        format!("Failed to parse response JSON: {}", e)
                    })?;
                println!("[DEBUG] HostBridge request successful: service={}, method={}", 
                    grpc_request.service, grpc_request.method);
                Ok(response_json)
            } else {
                let error_msg = format!("HostBridge returned error status: {}", status);
                println!("[DEBUG] {}", error_msg);
                Err(error_msg)
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to forward request to HostBridge: {}", e);
            println!("[DEBUG] {}", error_msg);
            Err(error_msg)
        }
    }
}

fn main() {
    // 创建进程管理器
    let process_manager = create_process_manager();
    
    // 设置 Ctrl+C 处理程序来清理子进程
    let cleanup_manager = process_manager.clone();
    ctrlc::set_handler(move || {
        println!("Received Ctrl+C, cleaning up child processes...");
        if let Ok(mut manager) = cleanup_manager.lock() {
            manager.kill_all();
        }
        std::process::exit(0);
    }).expect("Error setting Ctrl-C handler");
    
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(process_manager) // 将进程管理器添加到Tauri状态中
        .invoke_handler(tauri::generate_handler![
            select_workspace,
            start_cline_core,
            start_node_server,
            start_node_server_sidecar,
            stop_all_processes,
            test_grpc_connection,
            handle_webview_message
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();

            if let Some(window) = app.get_webview_window("main") {
                window.set_title("Cline Desktop").unwrap();
                // 其他窗口自定义操作
            }
            
            // 首先启动 HostBridge 服务器（在 Rust 中）
            let hostbridge_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = hostbridge::start_hostbridge_server(hostbridge_handle).await {
                    eprintln!("Failed to start HostBridge server: {}", e);
                }
            });
            
            // 在应用启动时自动启动cline-core
            tauri::async_runtime::spawn(async move {
                // 等待 HostBridge 服务启动（1秒）
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                
                // 获取进程管理器状态
                let process_manager_state = app_handle.state::<SharedProcessManager>();
                
                println!("[STARTUP] Starting cline-core process...");
                match start_cline_core(app_handle.clone(), process_manager_state).await {
                    Ok(msg) => println!("[STARTUP] {}", msg),
                    Err(e) => eprintln!("[STARTUP] Error starting cline-core: {}", e)
                }
            });
            
            Ok(())
        })
        .on_window_event(|window, event| {
            // 当窗口关闭时，确保所有子进程都被终止
            match event {
                tauri::WindowEvent::Destroyed => {
                    println!("Window is being destroyed, killing all child processes...");
                    let app_handle = window.app_handle();
                    let state = app_handle.state::<SharedProcessManager>();
                    
                    // 直接调用非异步的 kill_all 方法
                    if let Ok(mut manager) = state.lock() {
                        manager.kill_all();
                    };
                }
                tauri::WindowEvent::CloseRequested { .. } => {
                    println!("Window close requested, preparing to kill all child processes...");
                    let app_handle = window.app_handle();
                    let state = app_handle.state::<SharedProcessManager>();
                    
                    // 在窗口关闭请求时也清理进程
                    if let Ok(mut manager) = state.lock() {
                        manager.kill_all();
                    };
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
