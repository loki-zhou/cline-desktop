use std::pin::Pin;
use tauri::{AppHandle, Manager, Emitter};
use tauri_plugin_dialog::DialogExt;
use tonic::{transport::Server, Request, Response, Status};
use tokio_stream::{wrappers::ReceiverStream, Stream};

// 包含生成的 protobuf 代码
pub mod host {
    tonic::include_proto!("host");
}

pub mod cline {
    tonic::include_proto!("cline");
}

use host::*;

/// HostBridge 服务的主要实现 - 直接使用 Tauri API
#[derive(Clone)]
pub struct HostBridgeService {
    app_handle: AppHandle,
}

impl HostBridgeService {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

/// Window 服务实现 - 直接使用 Tauri 对话框 API
#[tonic::async_trait]
impl window_service_server::WindowService for HostBridgeService {
    async fn show_open_dialogue(
        &self,
        request: Request<ShowOpenDialogueRequest>,
    ) -> Result<Response<SelectedResources>, Status> {
        let req = request.into_inner();
        log::info!("HostBridge: show_open_dialogue called");
        
        // 直接使用 Tauri 的文件对话框
        let _dialog = self.app_handle.dialog().file();
        
        // 配置对话框选项 (暂时注释掉，因为需要异步处理)
        // if let Some(ref open_label) = req.open_label {
        //     dialog = dialog.set_title(open_label);
        // }
        
        // 执行文件选择
        // 注意：Tauri 对话框 API 是异步回调的，但 gRPC 接口需要同步返回
        // 这里我们先返回空结果，后续可以通过事件机制或其他方式实现
        
        log::info!("File dialog request: can_select_many={:?}, open_label={:?}", 
                   req.can_select_many, &req.open_label);
        
        // 目前返回空结果，实际应该通过其他机制实现文件选择
        Ok(Response::new(SelectedResources { paths: vec![] }))
    }

    async fn show_message(
        &self,
        request: Request<ShowMessageRequest>,
    ) -> Result<Response<SelectedResponse>, Status> {
        let req = request.into_inner();
        log::info!("HostBridge: show_message called with: {}", req.message);
        
        // 使用 Tauri 的消息对话框
        let dialog = self.app_handle.dialog().message(&req.message);
        
        // 这里我们可以直接调用 show() 或者根据类型选择不同的对话框
        dialog.show(|result| {
            log::info!("Message dialog result: {:?}", result);
        });
        
        Ok(Response::new(SelectedResponse { 
            selected_option: Some("ok".to_string()) 
        }))
    }

    async fn show_input_box(
        &self,
        request: Request<ShowInputBoxRequest>,
    ) -> Result<Response<ShowInputBoxResponse>, Status> {
        let req = request.into_inner();
        log::info!("HostBridge: show_input_box called");
        
        // Tauri 没有直接的输入框，我们可以通过自定义对话框实现
        // 或者发送事件到前端处理
        if let Some(window) = self.app_handle.get_webview_window("main") {
            let _ = window.emit("show-input-box", req.prompt.unwrap_or_default());
        }
        
        Ok(Response::new(ShowInputBoxResponse { 
            response: Some("".to_string()) 
        }))
    }

    async fn show_save_dialog(
        &self,
        _request: Request<ShowSaveDialogRequest>,
    ) -> Result<Response<ShowSaveDialogResponse>, Status> {
        log::info!("HostBridge: show_save_dialog called");
        
        // 使用 Tauri 的保存对话框 - 异步回调方式
        let selected_path: Option<String> = None; // 临时返回空结果
        
        // 注意：这里需要异步处理，但 gRPC 接口是同步的
        // 可能需要重新设计架构或使用不同的方法
        
        Ok(Response::new(ShowSaveDialogResponse { 
            selected_path 
        }))
    }

    async fn show_text_document(
        &self,
        request: Request<ShowTextDocumentRequest>,
    ) -> Result<Response<TextEditorInfo>, Status> {
        let req = request.into_inner();
        log::info!("HostBridge: show_text_document called for: {}", req.path);
        
        // 在桌面应用中，我们可以通过事件通知前端打开文档
        if let Some(window) = self.app_handle.get_webview_window("main") {
            let _ = window.emit("open-document", &req.path);
        }
        
        Ok(Response::new(TextEditorInfo {
            document_path: req.path,
            view_column: req.options.and_then(|o| o.view_column.map(|v| v as i32)),
            is_active: true,
        }))
    }

    async fn open_file(
        &self,
        request: Request<OpenFileRequest>,
    ) -> Result<Response<OpenFileResponse>, Status> {
        let req = request.into_inner();
        log::info!("HostBridge: open_file called for: {}", req.file_path);
        
        // 通知前端打开文件
        if let Some(window) = self.app_handle.get_webview_window("main") {
            let _ = window.emit("open-file", &req.file_path);
        }
        
        Ok(Response::new(OpenFileResponse {}))
    }

    async fn open_settings(
        &self,
        _request: Request<OpenSettingsRequest>,
    ) -> Result<Response<OpenSettingsResponse>, Status> {
        log::info!("HostBridge: open_settings called");
        
        // 通知前端打开设置
        if let Some(window) = self.app_handle.get_webview_window("main") {
            let _ = window.emit("open-settings", ());
        }
        
        Ok(Response::new(OpenSettingsResponse {}))
    }

    async fn get_open_tabs(
        &self,
        _request: Request<GetOpenTabsRequest>,
    ) -> Result<Response<GetOpenTabsResponse>, Status> {
        log::info!("HostBridge: get_open_tabs called");
        
        // 这需要从前端获取状态
        // 目前返回空列表
        Ok(Response::new(GetOpenTabsResponse { paths: vec![] }))
    }

    async fn get_visible_tabs(
        &self,
        _request: Request<GetVisibleTabsRequest>,
    ) -> Result<Response<GetVisibleTabsResponse>, Status> {
        log::info!("HostBridge: get_visible_tabs called");
        
        Ok(Response::new(GetVisibleTabsResponse { paths: vec![] }))
    }

    async fn get_active_editor(
        &self,
        _request: Request<GetActiveEditorRequest>,
    ) -> Result<Response<GetActiveEditorResponse>, Status> {
        log::info!("HostBridge: get_active_editor called");
        
        Ok(Response::new(GetActiveEditorResponse { 
            file_path: None 
        }))
    }
}

/// Workspace 服务实现
#[tonic::async_trait]
impl workspace_service_server::WorkspaceService for HostBridgeService {
    async fn get_workspace_paths(
        &self,
        _request: Request<GetWorkspacePathsRequest>,
    ) -> Result<Response<GetWorkspacePathsResponse>, Status> {
        log::info!("HostBridge: get_workspace_paths called");
        
        // 从应用状态或环境变量获取工作区路径
        let current_dir = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
            
        Ok(Response::new(GetWorkspacePathsResponse { 
            paths: vec![current_dir],
            id: None,
        }))
    }

    async fn save_open_document_if_dirty(
        &self,
        _request: Request<SaveOpenDocumentIfDirtyRequest>,
    ) -> Result<Response<SaveOpenDocumentIfDirtyResponse>, Status> {
        log::info!("HostBridge: save_open_document_if_dirty called");
        
        // 通知前端保存文档
        // if let Some(window) = self.app_handle.get_webview_window("main") {
        //     let _ = window.emit("save-document", &request.into_inner());
        // }
        
        Ok(Response::new(SaveOpenDocumentIfDirtyResponse { 
            was_saved: Some(false) 
        }))
    }

    async fn get_diagnostics(
        &self,
        _request: Request<GetDiagnosticsRequest>,
    ) -> Result<Response<GetDiagnosticsResponse>, Status> {
        log::info!("HostBridge: get_diagnostics called");
        
        // 诊断信息需要从前端或语言服务器获取
        Ok(Response::new(GetDiagnosticsResponse { 
            file_diagnostics: vec![] 
        }))
    }

    async fn search_workspace_items(
        &self,
        _request: Request<SearchWorkspaceItemsRequest>,
    ) -> Result<Response<SearchWorkspaceItemsResponse>, Status> {
        log::info!("HostBridge: search_workspace_items called");
        
        Ok(Response::new(SearchWorkspaceItemsResponse { 
            items: vec![] 
        }))
    }

    async fn open_problems_panel(
        &self,
        _request: Request<OpenProblemsPanelRequest>,
    ) -> Result<Response<OpenProblemsPanelResponse>, Status> {
        log::info!("HostBridge: open_problems_panel called");
        
        // 通知前端打开问题面板
        if let Some(window) = self.app_handle.get_webview_window("main") {
            let _ = window.emit("open-problems-panel", ());
        }
        
        Ok(Response::new(OpenProblemsPanelResponse {}))
    }

    async fn open_in_file_explorer_panel(
        &self,
        _request: Request<OpenInFileExplorerPanelRequest>,
    ) -> Result<Response<OpenInFileExplorerPanelResponse>, Status> {
        log::info!("HostBridge: open_in_file_explorer_panel called");
        
        // 通知前端打开文件浏览器面板
        // if let Some(window) = self.app_handle.get_webview_window("main") {
        //     let _ = window.emit("open-file-explorer", &request.into_inner());
        // }
        
        Ok(Response::new(OpenInFileExplorerPanelResponse {}))
    }
}

/// 环境服务实现
#[tonic::async_trait]
impl env_service_server::EnvService for HostBridgeService {
    async fn clipboard_write_text(
        &self,
        request: Request<cline::StringRequest>,
    ) -> Result<Response<cline::Empty>, Status> {
        let req = request.into_inner();
        log::info!("HostBridge: clipboard_write_text called");
        
        // 使用 Tauri 的剪贴板 API（如果有的话）
        // 或者通过系统调用实现
        if let Some(window) = self.app_handle.get_webview_window("main") {
            let _ = window.emit("clipboard-write", &req.value);
        }
        
        Ok(Response::new(cline::Empty {}))
    }

    async fn clipboard_read_text(
        &self,
        _request: Request<cline::EmptyRequest>,
    ) -> Result<Response<cline::String>, Status> {
        log::info!("HostBridge: clipboard_read_text called");
        
        // 读取剪贴板内容
        Ok(Response::new(cline::String { 
            value: "".to_string() // 待实现
        }))
    }

    async fn get_machine_id(
        &self,
        _request: Request<cline::EmptyRequest>,
    ) -> Result<Response<cline::String>, Status> {
        log::info!("HostBridge: get_machine_id called");
        
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
            
        Ok(Response::new(cline::String { 
            value: format!("cline-desktop-{}", hostname)
        }))
    }

    async fn get_host_version(
        &self,
        _request: Request<cline::EmptyRequest>,
    ) -> Result<Response<GetHostVersionResponse>, Status> {
        log::info!("HostBridge: get_host_version called");
        
        Ok(Response::new(GetHostVersionResponse {
            version: Some("1.0.0".to_string()),
            platform: Some("Cline Desktop".to_string()),
        }))
    }
}

/// 启动 HostBridge gRPC 服务器
pub async fn start_hostbridge_server(app_handle: AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:26041".parse()?;
    let service = HostBridgeService::new(app_handle);
    
    log::info!("Starting HostBridge gRPC server on {}", addr);
    
    // 创建健康检查服务
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter.set_serving::<window_service_server::WindowServiceServer<HostBridgeService>>().await;
    health_reporter.set_serving::<workspace_service_server::WorkspaceServiceServer<HostBridgeService>>().await;
    health_reporter.set_serving::<env_service_server::EnvServiceServer<HostBridgeService>>().await;
    
    Server::builder()
        .add_service(health_service)
        .add_service(window_service_server::WindowServiceServer::new(service.clone()))
        .add_service(workspace_service_server::WorkspaceServiceServer::new(service.clone()))
        .add_service(env_service_server::EnvServiceServer::new(service.clone()))
        .add_service(diff_service_server::DiffServiceServer::new(service.clone()))
        .add_service(watch_service_server::WatchServiceServer::new(service.clone()))
        .add_service(testing_service_server::TestingServiceServer::new(service.clone()))
        .serve(addr)
        .await?;
    
    Ok(())
}

/// 差异服务实现
#[tonic::async_trait]
impl diff_service_server::DiffService for HostBridgeService {
    async fn open_diff(
        &self,
        _request: Request<OpenDiffRequest>,
    ) -> Result<Response<OpenDiffResponse>, Status> {
        log::info!("HostBridge: open_diff called");
        
        Ok(Response::new(OpenDiffResponse {
            diff_id: Some(format!("diff-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())),
        }))
    }

    async fn get_document_text(
        &self,
        _request: Request<GetDocumentTextRequest>,
    ) -> Result<Response<GetDocumentTextResponse>, Status> {
        log::info!("HostBridge: get_document_text called");
        
        Ok(Response::new(GetDocumentTextResponse {
            content: Some("".to_string()),
        }))
    }

    async fn replace_text(
        &self,
        _request: Request<ReplaceTextRequest>,
    ) -> Result<Response<ReplaceTextResponse>, Status> {
        log::info!("HostBridge: replace_text called");
        
        Ok(Response::new(ReplaceTextResponse {}))
    }

    async fn scroll_diff(
        &self,
        _request: Request<ScrollDiffRequest>,
    ) -> Result<Response<ScrollDiffResponse>, Status> {
        log::info!("HostBridge: scroll_diff called");
        
        Ok(Response::new(ScrollDiffResponse {}))
    }

    async fn truncate_document(
        &self,
        _request: Request<TruncateDocumentRequest>,
    ) -> Result<Response<TruncateDocumentResponse>, Status> {
        log::info!("HostBridge: truncate_document called");
        
        Ok(Response::new(TruncateDocumentResponse {}))
    }

    async fn save_document(
        &self,
        _request: Request<SaveDocumentRequest>,
    ) -> Result<Response<SaveDocumentResponse>, Status> {
        log::info!("HostBridge: save_document called");
        
        Ok(Response::new(SaveDocumentResponse {}))
    }

    async fn close_all_diffs(
        &self,
        _request: Request<CloseAllDiffsRequest>,
    ) -> Result<Response<CloseAllDiffsResponse>, Status> {
        log::info!("HostBridge: close_all_diffs called");
        
        Ok(Response::new(CloseAllDiffsResponse {}))
    }

    async fn open_multi_file_diff(
        &self,
        _request: Request<OpenMultiFileDiffRequest>,
    ) -> Result<Response<OpenMultiFileDiffResponse>, Status> {
        log::info!("HostBridge: open_multi_file_diff called");
        
        Ok(Response::new(OpenMultiFileDiffResponse {}))
    }
}

/// 监视服务实现
#[tonic::async_trait]
impl watch_service_server::WatchService for HostBridgeService {
    type subscribeToFileStream = Pin<Box<dyn Stream<Item = Result<FileChangeEvent, Status>> + Send>>;

    async fn subscribe_to_file(
        &self,
        _request: Request<SubscribeToFileRequest>,
    ) -> Result<Response<Self::subscribeToFileStream>, Status> {
        log::info!("HostBridge: subscribe_to_file called");
        
        // 创建一个空的流
        let (_tx, rx) = tokio::sync::mpsc::channel(1);
        let stream = ReceiverStream::new(rx);
        
        Ok(Response::new(Box::pin(stream)))
    }
}

/// 测试服务实现
#[tonic::async_trait]
impl testing_service_server::TestingService for HostBridgeService {
    async fn get_webview_html(
        &self,
        _request: Request<GetWebviewHtmlRequest>,
    ) -> Result<Response<GetWebviewHtmlResponse>, Status> {
        log::info!("HostBridge: get_webview_html called");
        
        Ok(Response::new(GetWebviewHtmlResponse {
            html: Some("<html><body>Cline Desktop Webview</body></html>".to_string()),
        }))
    }
}