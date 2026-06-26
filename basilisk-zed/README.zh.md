<p align="center"><a href="README.md">English</a> · <strong>简体中文</strong></p>

> 📝 本文档由机器翻译生成，欢迎母语者校对改进。

# basilisk-zed

Basilisk 的 Zed 编辑器扩展 —— 基于 WASM 的 Python 类型检查与语言服务器集成。

<p align="center">
  <img src="images/screenshot.png" alt="Basilisk in action — type checking, diagnostics, and refactoring in the editor" width="900">
</p>

## 在 Basilisk 中的角色

这是 **Zed 编辑器集成**。它是一个编译为 WASM 的原生 Zed 扩展，将 Basilisk 语言服务器连接到 Zed，提供实时诊断、悬停提示、跳转到定义、代码操作，以及通过 DAP 实现的调试。

## 核心概念

- **WASM 扩展** —— 编译为面向 `wasm32-wasip1` 的 `cdylib` crate，由 Zed 原生加载。
- **`zed_extension_api`** —— 使用 Zed 官方扩展 API 管理语言服务器生命周期。
- **`basilisk-common`** —— 与 Basilisk 工作区的其余部分共享诊断代码和常量（同样兼容 WASM）。
- **Tree-sitter 语法** —— 通过 tree-sitter 提供 Python 语法高亮。
- **DAP 调试** —— 支持 Debug Adapter Protocol，实现集成的 Python 调试。

## 构建

```sh
make package-zed
```

## 依赖

| Crate | 用途 |
|-------|---------|
| `zed_extension_api` | Zed 扩展 API |
| `basilisk-common` | 共享的常量和类型 |

## 状态

第 2 阶段 —— 扩展结构已完成，正在连接到 Basilisk LSP。

## 许可证

MIT。
