fn main() {
    tauri_build::build();
    
    // 生成 gRPC 代码
    let proto_files = [
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
        .build_client(false) // 我们只需要服务端
        .compile(&proto_files, &proto_include_dirs)
        .unwrap_or_else(|e| panic!("Failed to compile protos: {}", e));
}
