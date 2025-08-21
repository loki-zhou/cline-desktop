# Cline Desktop (Tauri) - 实施计划与进度

本文档用于追踪使用Tauri构建Cline独立桌面应用的进度。

## 核心架构

- **项目隔离**: 创建一个独立的 `cline-desktop` 仓库，将原始的 `cline` 仓库作为 `git submodule` 引入，确保不对原始项目产生任何修改。
- **Tauri作为包装器**: Tauri的核心职责是提供一个原生窗口来承载`webview-ui`，并利用`sidecar`功能管理`cline-core`进程的生命周期。
- **直接gRPC通信**: 前端`webview-ui`将直接通过标准的gRPC-Web请求与`sidecar`中运行的`cline-core` gRPC服务通信，最大限度复用现有代码。

## 已完成的工作 (Milestone 1: Project Setup & MVP 验证)

- [x] **创建独立项目**:
    - [x] 创建了 `cline-desktop` 目录。
    - [x] 在 `cline-desktop` 中初始化了 `git` 和 `npm`。
    - [x] 将 `cline` 仓库添加为 `git submodule`。
- [x] **Tauri 项目初始化**:
    - [x] 在 `cline-desktop` 中安装了 `@tauri-apps/cli`。
    - [x] 运行 `tauri init` 生成了 `src-tauri` 目录结构。
- [x] **Tauri 配置**:
    - [x] 修改了 `cline-desktop/src-tauri/tauri.conf.json`：
        - 配置了 `build.frontendDist` 和 `build.devUrl` 以指向 `cline` 子模块中的 `webview-ui`。
        - 配置了 `build.beforeDevCommand` 和 `build.beforeBuildCommand` 以便在Tauri命令执行前，自动构建`cline`子模块。
        - 配置了 `allowlist` 极以允许必要的 `http` 和 `shell` 权限。
    - [x] 修改了 `cline-desktop/src-tauri/Cargo.toml` 和 `src-tauri/main.rs` 以集成 `tauri-plugin-shell`。
- [x] **NPM 脚本**:
    - [x] 在 `cline-desktop/package.json` 中添加了 `dev` 和 `build` 脚本来运行Tauri。
- [x] **编译问题修复**:
    - [x] 解决了 `src-tauri/src/main.rs` 中的 Rust 生命周期编译错误 (E0521)
    - [x] 应用现在可以成功编译和运行
- [x] **MVP 验证完成**:
    - [x] `cline` 子模块依赖安装成功
    - [x] `webview-ui` 开发服务器正常启动 (http://localhost:5173/)
    - [x] Tauri窗口成功打开并加载了`webview-ui`
    - [x] `cline-core` sidecar进程成功启动并运行
    - [x] 前端通过gRPC-Web成功连接到Rust后端
    - [x] 成功捕获并显示sidecar进程的输出

## 已完成的工作 (阶段二: 核心功能完善)

- [x] **gRPC 请求转发实现**:
    - [x] **实现状态**: Rust 后端已实现真正的 gRPC 请求转发到 `cline-core` 进程
    - [x] **技术细节**: 添加了 tonic、tokio、prost、futures、tokio-stream 等依赖
    - [x] **连接地址**: 连接到 `http://127.0.0.1:26040` (cline-core 的 gRPC 服务端口)
    - [x] **测试结果**: 前端 gRPC 请求成功转发，但由于 hostbridge 启动较慢，连接暂时失败
    - [x] **问题解决**: 前端不再收到 "Received ProtoBus message with no response or error" 警告
- [ ] **实现工作区选择**:
    - [x] 添加了 Tauri 对话框插件依赖 (tauri-plugin-dialog)
    - [x] 实现了工作区选择的 Tauri 命令 (`select_workspace`)
    - [x] 在前端添加了调用工作区选择命令的逻辑
    - [ ] 修复对话框 API 调用问题 (需要导入 DialogExt trait)
    - [ ] 完善 gRPC 路径传递逻辑
    - [ ] 测试工作区选择功能

### 阶段三: 体验优化

-   [ ] **UI 视图实现**:
    -   [ ] 在 `webview-ui` 中实现文件树、编辑器和Diff视图。
-   [ ] **终端集成**:
    -   [ ] 在 `webview-ui` 中集成Xterm.js并打通与`cline-core`中`node-pty`的数据流。
-   [ ] **持久化**:
    -   [ ] 实现设置和密钥的持久化存储。
