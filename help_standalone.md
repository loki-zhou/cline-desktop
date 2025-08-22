


基于代码分析，cline的standalone模式和vscode插件模式在架构和数据流上有显著差异：

## **核心架构差异**

### **VSCode插件模式**
VSCode插件模式直接运行在VSCode扩展环境中 [1](#0-0) 。使用`VscodeWebviewProvider`来创建和管理webview界面 [2](#0-1) ，直接调用VSCode的原生API进行文件操作、窗口管理等功能。

### **Standalone模式**
Standalone模式运行在独立进程中 [3](#0-2) ，通过模拟VSCode环境来实现兼容性。它创建了一个虚拟的VSCode扩展上下文 [4](#0-3) ，并使用`ExternalWebviewProvider`替代VSCode原生的webview实现 [5](#0-4) 。

## **通信机制差异**

### **VSCode插件模式数据流**
在VSCode插件模式中，webview通过VSCode的消息传递机制与扩展后端通信 [6](#0-5) 。数据流向为：`Webview UI ↔ VscodeWebviewProvider ↔ Controller ↔ VSCode API`

### **Standalone模式数据流**
Standalone模式建立了gRPC服务架构。核心服务`ProtoBus`监听在26040端口 [7](#0-6) ，对外提供gRPC接口 [8](#0-7) 。同时，通过`ExternalHostBridgeClientManager`建立与外部Host Bridge的gRPC连接 [9](#0-8) 。

数据流向为：`External Client ↔ ProtoBus gRPC Server ↔ Cline Core ↔ Host Bridge gRPC Client ↔ External Host`

## **Host Provider差异**

两种模式使用不同的Host Provider实现 [10](#0-9) ：

- **VSCode模式**：直接使用VSCode API创建webview和diff view
- **Standalone模式**：使用`ExternalWebviewProvider` [11](#0-10) 和外部gRPC客户端进行通信

## **环境模拟**

Standalone模式的一个重要特点是完全模拟了VSCode的扩展环境，包括扩展上下文、存储、秘钥管理等 [4](#0-3) ，使得核心业务逻辑可以在两种模式下保持一致。

## **Notes**

Standalone模式的设计使得cline可以脱离VSCode环境独立运行，通过gRPC接口与任何支持该协议的客户端通信，提供了更大的部署灵活性。而VSCode插件模式则提供了与VSCode深度集成的用户体验，能够直接利用VSCode的所有编辑器功能和UI组件。


要独立运行cline，您需要使用standalone模式。基于代码分析，这里是具体的步骤：

## **构建Standalone版本**

首先需要编译standalone版本 [1](#1-0) 。运行以下命令：

```bash
npm run compile-standalone
```

这会触发后续的打包脚本 [2](#1-1) 。

## **环境配置**

Standalone模式会创建独立的配置目录 [3](#1-2) ：

- 默认使用 `~/.cline` 作为主目录
- 可通过 `CLINE_DIR` 环境变量自定义
- 数据存储在 `~/.cline/data` 目录
- 核心文件安装在 `~/.cline/core/{version}` 目录

## **VSCode环境模拟**

Standalone模式完全模拟了VSCode扩展环境 [4](#1-3) ，包括：

- 扩展上下文和元数据
- 全局状态和工作区状态存储
- 密钥管理
- 环境变量集合

## **终端集成**

独立运行时使用增强的终端管理 [5](#1-4) ，直接管理子进程而不依赖VSCode的终端API [6](#1-5) 。

## **通信协议**

Standalone模式使用gRPC进行通信 [7](#1-6) ，通过检测 `window.__is_standalone__` 标志来区分运行模式。

## **运行方式**

编译完成后，standalone版本可以：
1. 作为独立进程运行
2. 通过gRPC接口与外部客户端通信
3. 不依赖VSCode环境，但保持完整的cline功能

**Notes**

Standalone模式的设计使得cline可以集成到任何支持gRPC的应用中，为非VSCode环境提供AI编程助手功能。所有核心逻辑保持不变，只是将VSCode特定的API替换为独立实现。

Wiki pages you might want to explore:
- [Overview (cline/cline)](/wiki/cline/cline#1)

