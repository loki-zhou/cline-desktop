# Cline Desktop 架构改进

## 原始架构

原始架构中，通信流程如下：

```
webview-ui → Tauri Rust → gRPC → cline-core
```

这种架构存在以下问题：

1. **通信链路复杂**：前端需要通过Tauri的Rust后端作为中间代理与cline-core通信
2. **代码重复**：Rust后端实现了gRPC客户端逻辑，但这些逻辑在webview-ui中已经存在
3. **不符合Tauri最佳实践**：增加了不必要的复杂性和潜在的故障点

## 改进后的架构

改进后的架构如下：

```
webview-ui → 直接gRPC → cline-core
```

Tauri的Rust后端只负责：
1. 启动和管理cline-core进程的生命周期
2. 提供文件系统访问等Tauri原生功能

## 主要改动

1. **简化Rust后端**：
   - 移除了gRPC代理逻辑
   - 只保留了进程管理和文件系统操作功能

2. **配置更新**：
   - 更新了tauri.conf.json中的HTTP allowlist，确保前端可以直接访问gRPC服务

3. **前端直接通信**：
   - 前端可以直接使用gRPC-Web与cline-core通信
   - 参考grpc-web-example.js中的示例代码

## 优势

1. **架构简化**：减少了通信环节，降低了复杂性
2. **代码复用**：充分利用cline已有的standalone模式设计
3. **性能提升**：减少了数据转发环节，提高了通信效率
4. **维护性提高**：前端与后端逻辑更加清晰分离

## 后续工作

1. 在前端实现完整的gRPC-Web客户端，直接与cline-core通信
2. 完善文件系统操作等需要Tauri原生功能的部分
3. 实现工作区选择功能，并将选择的路径传递给cline-core