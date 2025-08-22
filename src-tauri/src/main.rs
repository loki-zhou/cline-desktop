// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod hostbridge;

use std::sync::{Arc, Mutex};
use tauri::{Manager, Emitter};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::{CommandEvent, CommandChild};

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
        self.processes.push(child);
    }

    fn kill_all(&mut self) {
        let mut processes_to_kill = Vec::new();
        std::mem::swap(&mut processes_to_kill, &mut self.processes);
        
        for mut process in processes_to_kill {
            if let Err(e) = process.kill() {
                eprintln!("Failed to kill process: {}", e);
            }
        }
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
                        println!("cline-core stdout: {}", String::from_utf8_lossy(&line));
                        window
                            .emit("cline-stdout", String::from_utf8_lossy(&line).to_string())
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
fn stop_all_processes(process_manager: tauri::State<'_, SharedProcessManager>) -> Result<String, String> {
    println!("Stopping all child processes...");
    
    // 使用作用域来确保锁在函数结束前被释放
    {
        let mut manager = process_manager.lock().unwrap();
        manager.kill_all();
    }
    
    Ok("All processes stopped successfully".to_string())
}

fn main() {
    // 创建进程管理器
    let process_manager = create_process_manager();
    
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(process_manager) // 将进程管理器添加到Tauri状态中
        .invoke_handler(tauri::generate_handler![
            select_workspace,
            start_cline_core,
            start_node_server,
            start_node_server_sidecar,
            stop_all_processes
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            
            // 首先启动 HostBridge 服务器（在 Rust 中）
            let hostbridge_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = hostbridge::start_hostbridge_server(hostbridge_handle).await {
                    eprintln!("Failed to start HostBridge server: {}", e);
                }
            });
            
            // 在应用启动时自动启动cline-core
            tauri::async_runtime::spawn(async move {
                // 获取进程管理器状态
                let process_manager_state = app_handle.state::<SharedProcessManager>();
                
                match start_cline_core(app_handle.clone(), process_manager_state).await {
                    Ok(msg) => println!("{}", msg),
                    Err(e) => eprintln!("Error starting cline-core: {}", e)
                }
            });
            
            Ok(())
        })
        .on_window_event(|window, event| {
            // 当窗口关闭时，确保所有子进程都被终止
            if let tauri::WindowEvent::Destroyed = event {
                println!("Window is being destroyed, killing all child processes...");
                let app_handle = window.app_handle();
                let state = app_handle.state::<SharedProcessManager>();
                
                // 直接调用非异步的 kill_all 方法
                let mut manager = state.lock().unwrap();
                manager.kill_all();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
