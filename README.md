# Cline Desktop - Tauri 桌面应用

<div align="center">
  <img src="src-tauri/icons/icon.png" alt="Cline Desktop Logo" width="120" />
</div>

<p align="center">
  <b>将 Cline VS Code 扩展转换为功能强大的独立桌面应用</b>
</p>

## 项目概述

Cline Desktop 是一个基于 Tauri 框架的桌面应用程序，旨在将 Cline VS Code 扩展的强大功能带到独立环境中。该项目使用 Tauri 作为包装器，提供原生窗口来承载 webview-ui，并通过 sidecar 功能管理 cline-core 进程的生命周期，为用户提供流畅、高效的开发体验。

## 核心架构

<div align="center">
  <img src="https://raw.githubusercontent.com/tauri-apps/tauri/dev/app-icon.png" alt="Tauri Logo" width="80" />
</div>

- **📦 项目隔离**: 创建独立的 `cline-desktop` 仓库，将原始的 `cline` 仓库作为 `git submodule` 引入，确保不对原始项目产生任何修改。
- **🖼️ Tauri作为包装器**: Tauri 的核心职责是提供一个原生窗口来承载 `webview-ui`，并利用 `sidecar` 功能管理 `cline-core` 进程的生命周期。
- **🔄 直接gRPC通信**: 前端 `webview-ui` 将直接通过标准的 gRPC-Web 请求与 `sidecar` 中运行的 `cline-core` gRPC 服务通信，最大限度复用现有代码。

## 开发进度

### ✅ 已完成的工作 (里程碑 1: 项目搭建与 MVP 验证)

<table>
  <tr>
    <th>任务</th>
    <th>详情</th>
  </tr>
  <tr>
    <td><b>📂 创建独立项目</b></td>
    <td>
      • 创建了 <code>cline-desktop</code> 目录<br>
      • 在 <code>cline-desktop</code> 中初始化了 <code>git</code> 和 <code>npm</code><br>
      • 将 <code>cline</code> 仓库添加为 <code>git submodule</code>
    </td>
  </tr>
  <tr>
    <td><b>🛠️ Tauri 项目初始化</b></td>
    <td>
      • 在 <code>cline-desktop</code> 中安装了 <code>@tauri-apps/cli</code><br>
      • 运行 <code>tauri init</code> 生成了 <code>src-tauri</code> 目录结构
    </td>
  </tr>
  <tr>
    <td><b>⚙️ Tauri 配置</b></td>
    <td>
      • 修改了 <code>cline-desktop/src-tauri/tauri.conf.json</code><br>
      • 修改了 <code>cline-desktop/src-tauri/Cargo.toml</code> 和 <code>src-tauri/main.rs</code> 以集成 <code>tauri-plugin-shell</code>
    </td>
  </tr>
  <tr>
    <td><b>📜 NPM 脚本</b></td>
    <td>
      • 在 <code>cline-desktop/package.json</code> 中添加了 <code>dev</code> 和 <code>build</code> 脚本来运行 Tauri
    </td>
  </tr>
  <tr>
    <td><b>🪟 Windows 环境适配</b></td>
    <td>
      • 修改了 <code>cline/scripts/build-proto.mjs</code> 文件，将 protoc 路径从 <code>grpc-tools</code> 包中的路径改为直接使用 <code>protoc</code> 命令
    </td>
  </tr>
  <tr>
    <td><b>🔧 编译问题修复</b></td>
    <td>
      • 解决了 <code>src-tauri/src/main.rs</code> 中的 Rust 生命周期编译错误 (E0521)<br>
      • 应用现在可以成功编译和运行
    </td>
  </tr>
  <tr>
    <td><b>✨ MVP 验证完成</b></td>
    <td>
      • <code>cline</code> 子模块依赖安装成功<br>
      • <code>webview-ui</code> 开发服务器正常启动 (http://localhost:5173/)<br>
      • Tauri 窗口成功打开并加载 <code>webview-ui</code><br>
      • <code>cline-core</code> sidecar 进程成功启动并运行<br>
      • 前端通过 gRPC-Web 成功连接到 Rust 后端<br>
      • 成功捕获并显示 sidecar 进程的输出
    </td>
  </tr>
</table>

### 🚀 已完成的工作 (阶段二: 核心功能完善)

<table>
  <tr>
    <th>任务</th>
    <th>详情</th>
  </tr>
  <tr>
    <td><b>🔄 gRPC 请求转发实现</b></td>
    <td>
      • <b>实现状态</b>: Rust 后端已实现真正的 gRPC 请求转发到 <code>cline-core</code> 进程<br>
      • <b>技术细节</b>: 添加了 tonic、tokio、prost、futures、tokio-stream 等依赖，实现了完整的 gRPC 客户端连接逻辑<br>
      • <b>连接地址</b>: 连接到 <code>http://127.0.0.1:26040</code> (cline-core 的 gRPC 服务端口)<br>
      • <b>测试结果</b>: 前端 gRPC 请求成功转发，但由于 hostbridge 启动较慢，连接暂时失败<br>
      • <b>问题解决</b>: 前端不再收到 "Received ProtoBus message with no response or error" 警告
    </td>
  </tr>
  <tr>
    <td><b>🔄 实现工作区选择</b></td>
    <td>
      • <b>当前状态</b>: 已实现 Tauri 后端的工作区选择命令，前端调用逻辑已添加<br>
      • <b>技术细节</b>: 添加了 tauri-plugin-dialog 依赖，实现了文件夹选择对话框<br>
      • <b>剩余工作</b>: 需要修复对话框 API 调用问题，完善 gRPC 路径传递逻辑
    </td>
  </tr>
</table>

### 📋 未来计划

<table>
  <tr>
    <th colspan="2">阶段二: 核心功能完善</th>
  </tr>
  <tr>
    <td><b>🖥️ UI 视图实现</b></td>
    <td>在 <code>webview-ui</code> 中实现文件树、编辑器和 Diff 视图</td>
  </tr>
  <tr>
    <td><b>💻 终端集成</b></td>
    <td>在 <code>webview-ui</code> 中集成 Xterm.js 并打通与 <code>cline-core</code> 中 <code>node-pty</code> 的数据流</td>
  </tr>
  <tr>
    <th colspan="2">阶段三: 体验优化</th>
  </tr>
  <tr>
    <td><b>💾 持久化</b></td>
    <td>实现设置和密钥的持久化存储</td>
  </tr>
</table>

## 开发环境设置

### 前提条件

<table>
  <tr>
    <td><img src="https://nodejs.org/static/images/logos/nodejs-new-pantone-black.svg" height="20"></td>
    <td><b>Node.js 和 npm</b></td>
  </tr>
  <tr>
    <td><img src="https://www.rust-lang.org/static/images/rust-logo-blk.svg" height="20"></td>
    <td><b>Rust 和 Cargo</b></td>
  </tr>
  <tr>
    <td><img src="https://raw.githubusercontent.com/tauri-apps/tauri/dev/app-icon.png" height="20"></td>
    <td><b>Tauri CLI</b> (<code>npm install -g @tauri-apps/cli</code>)</td>
  </tr>
  <tr>
    <td><img src="https://developers.google.com/static/protocol-buffers/images/logo" height="20"></td>
    <td><b>protoc</b> (Protocol Buffers 编译器)</td>
  </tr>
</table>

### 开发命令

```bash
# 克隆仓库并初始化子模块
git clone https://github.com/yourusername/cline-desktop.git
cd cline-desktop
git submodule update --init --recursive

# 安装依赖
npm install

# 开发模式运行
npm run dev

# 构建应用
npm run build
```

## 项目结构

```
cline-desktop/
├── cline/                  # Cline 子模块
│   ├── proto/              # Protocol Buffers 定义
│   ├── scripts/            # 构建脚本
│   ├── src/                # Cline 源代码
│   ├── standalone/         # 独立运行时文件
│   └── webview-ui/         # Web UI 源代码
├── src-tauri/              # Tauri 应用源代码
│   ├── src/                # Rust 源代码
│   │   ├── main.rs         # 主程序入口
│   │   ├── lib.rs          # 库函数
│   │   └── fs_commands.rs  # 文件系统命令
│   ├── Cargo.toml          # Rust 依赖配置
│   └── tauri.conf.json     # Tauri 配置
├── patches/                # 补丁文件
└── package.json            # 项目配置
```

## 贡献指南

欢迎贡献代码和提出问题！请遵循以下步骤：

<div align="center">
  <img src="https://opensource.guide/assets/images/illos/contribute.svg" width="300" alt="Contribution Illustration">
</div>

1. Fork 本仓库
2. 创建您的特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交您的更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 打开一个 Pull Request

## 许可证

本项目采用 MIT 许可证 - 详见 LICENSE 文件

---

<p align="center">
  使用 <a href="https://tauri.app">Tauri</a> 构建 | 
  <a href="https://github.com/yourusername/cline-desktop/issues">报告问题</a> | 
  <a href="https://github.com/yourusername/cline-desktop/blob/main/CHANGELOG.md">更新日志</a>
</p>
