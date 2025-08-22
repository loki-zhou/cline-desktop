fn main() {
    tauri_build::build();
    
    // 生成 HostBridge gRPC 服务端代码
    let host_proto_files = [
        "../cline/proto/host/window.proto",
        "../cline/proto/host/workspace.proto", 
        "../cline/proto/host/env.proto",
        "../cline/proto/host/diff.proto",
        "../cline/proto/host/watch.proto",
        "../cline/proto/host/testing.proto",
    ];
    
    let proto_include_dirs = ["../cline/proto"];
    
    tonic_build::configure()
        .build_server(true)
        .build_client(false) // HostBridge 只需要服务端
        .compile(&host_proto_files, &proto_include_dirs)
        .unwrap_or_else(|e| panic!("Failed to compile host protos: {}", e));
        
    // 生成 Cline gRPC 客户端代码
    let cline_proto_files = [
        "../cline/proto/cline/common.proto",
        "../cline/proto/cline/state.proto",
        "../cline/proto/cline/ui.proto",
        "../cline/proto/cline/mcp.proto",
        "../cline/proto/cline/file.proto",
        "../cline/proto/cline/models.proto",
        "../cline/proto/cline/task.proto",
        "../cline/proto/cline/account.proto",
        "../cline/proto/cline/browser.proto",
        "../cline/proto/cline/commands.proto",
        "../cline/proto/cline/checkpoints.proto",
        "../cline/proto/cline/slash.proto",
        "../cline/proto/cline/web.proto",
    ];
    
    tonic_build::configure()
        .build_server(false)
        .build_client(true) // Cline 我们需要客户端
        .compile(&cline_proto_files, &proto_include_dirs)
        .unwrap_or_else(|e| panic!("Failed to compile cline protos: {}", e));
}
