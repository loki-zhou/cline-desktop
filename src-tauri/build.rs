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
}
