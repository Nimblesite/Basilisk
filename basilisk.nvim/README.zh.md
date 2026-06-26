<p align="center"><a href="README.md">English</a> · <strong>简体中文</strong></p>

> 📝 本文档由机器翻译生成，欢迎母语者校对改进。

# basilisk.nvim

为 Basilisk 打造的一流 Neovim 插件 —— 零配置的 Python 类型检查、调试、性能分析与测试探索。

<p align="center">
  <img src="images/screenshot.png" alt="Basilisk in action — type checking, diagnostics, and refactoring in the editor" width="900">
</p>

## 在 Basilisk 中的角色

这是 **Neovim 编辑器集成**。它将 Neovim 内置的 LSP 客户端连接到 Basilisk 语言服务器，提供与 VS Code 扩展相同的功能集：实时诊断、悬停信息、跳转到定义、代码操作、内嵌提示（inlay hints）、集成调试以及性能分析。

## 功能特性

- **零配置安装** —— 自动检测 `basilisk` 二进制文件并建立连接
- **实时诊断** —— 错误在你输入时即时内联显示
- **跳转到定义、悬停信息、查找引用** —— 完整的 LSP 导航
- **代码操作与重构** —— 提取、重命名、移动、内联
- **内嵌提示（inlay hints）** —— 参数名称与推断类型
- **集成调试** —— 兼容 nvim-dap，按 F5 即可调试
- **测试浏览器** —— 在编辑器中发现并运行 pytest 测试
- **Python 性能分析** —— 直接在编辑器中查看 py-spy 热力图
- **内存泄漏追踪** —— 在开发过程中检测泄漏
- **uv 集成** —— `uv sync` 与 `uv add` 命令
- **状态栏** —— 在状态栏中显示 LSP 状态
- **健康检查** —— `:checkhealth basilisk` 进行诊断

## 要求

- Neovim 0.10+
- PATH 中包含 `basilisk` 二进制文件（或显式配置）

## 快速开始

```lua
require("basilisk").setup()
```

完整文档请参阅 [doc/basilisk.txt](doc/basilisk.txt)。

## 许可证

MIT。
