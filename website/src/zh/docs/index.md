---
layout: layouts/docs.njk
title: 简介
description: Basilisk 是什么，为什么存在，以及它如何作为完全开源的 Python 语言服务器替代 Pylance 和 Pyright。
keywords: basilisk, python, 语言服务器, pylance替代, pyright, 类型检查器, lsp, vs code, 严格, rust
lang: zh
---

# 简介

Basilisk 是一个**完整的 Python 语言服务器**，替代 Pylance 和 Pyright。Pylance 所做的一切——自动补全、跳转到定义、悬停信息、重构、诊断、集成调试、性能分析——Basilisk 也做，完全开源，默认严格模式。

它不仅仅是一个类型检查器。它是一个功能完整的 LSP，为 **VS Code**、**Neovim** 和 **Zed** 提供一流扩展——以及任何支持语言服务器协议的其他编辑器。无专有扩展。无 Node.js。单个 Rust 二进制文件。

## Basilisk 解决的问题

Pylance 是 VS Code 中使用最广泛的 Python 扩展。它也是**专有的**——您无法检查、修改或重新发布它。Pyright，底层的开源类型检查器，很强大，但它*只是*一个类型检查器——没有专有 Pylance 包装，它不提供补全、悬停、跳转到定义或重构。

其他每个 Python 类型检查器（mypy、ty、Pyrefly）都默认使用*渐进类型*。未类型化的代码静默通过。`Any` 在类型推断中扩散而不发出警告。严格性是您必须故意选择加入的，配置，记住在 CI 中强制执行，并向每个新团队成员重新解释的。

Basilisk 采取不同的立场。**它替换整个 Pylance 堆栈**——类型检查、语言功能和 VS Code 集成——以一个默认严格的开源替代品。类型注解是契约，不是文档。

## Basilisk 是什么

- **功能完整的语言服务器** (LSP)——自动补全、跳转到定义、悬停、查找引用、重命名、[16 个重构操作](/zh/docs/refactoring/)、代码操作、内联提示
- **VS Code、Neovim (0.10+) 和 Zed 的编辑器扩展**——安装它，禁用 Pylance，一切正常工作
- **集成调试器**——按 F5 调试 Python，支持断点、单步执行、变量检查和监视表达式，全部通过 Basilisk LSP 代理
- **集成性能分析器**——通过 py-spy 进行 CPU 分析，具有内联热图注解、火焰图、内存泄漏检测和引用图可视化，全部在您的编辑器内
- **默认严格的类型检查器**——无 `--strict` 标志，无渐进模式，无需选择加入
- **用于 CI 集成的 CLI 工具**——发现错误时以代码 1 退出
- **迁移助手**，读取您现有的 `pyrightconfig.json` 或 `mypy.ini`
- **uv 集成**——工作区检测、锁文件解析和包管理命令
- 用 **Rust** 编写——作为单个二进制文件发布，没有运行时依赖

## Basilisk 不是什么

- 不是编译器——您的 Python 代码照常在 CPython 上运行
- 不是运行时类型检查器——分析在开发时静态发生
- 不是 Mojo 依赖——Basilisk 的所有权注解今天就可以与标准 Python 一起使用

## 只有一种模式

Basilisk 有一种单一的操作模式。没有 `--basic`、`--standard` 或 `--permissive` 标志。这是故意的。

当严格性是选择加入的时候，团队会向宽松的默认值漂移。截止日期来临。技术债务积累。`--strict` 标志从未被添加到 CI 脚本中。Basilisk 完全消除了这种可能性。

选择退出仍然是可能的——对于遗留目录，通过每路径配置和一个可选的到期截止日期：

```toml
[tool.basilisk.per-path-overrides."legacy/**"]
strict = false
deadline = "2026-12-31"
```

这承认大型代码库不能在一夜之间完全类型化，同时确保宽松期有到期日期。

## Mojo 启发的安全性

Basilisk 将 Mojo 启发的所有权语义作为静态分析注解添加到标准 Python 语法上。使用 `typing` 模块中的 `Annotated`，您可以声明一个参数是：

- **`Borrowed`** — 只读引用；变异是类型错误
- **`InOut`** — 可变引用；必须明确声明
- **`Owned`** — 所有权已转移；转移后使用是类型错误

这些不是运行时构造。它们是静态检查的注解。通过 Basilisk 所有权检查的代码在结构上与 Mojo 的类型期望兼容。

## 项目状态

Basilisk 目前处于 **v0.1.0**——核心检查器、LSP 服务器和 VS Code 扩展都在工作。自动补全、跳转到定义、悬停、诊断和内联提示今天就在发布。

| 阶段 | 里程碑 | 状态 |
|---|---|---|
| 1 | 解析器、解析器、类型检查器、CLI | 完成 |
| 2 | LSP 服务器、VS Code 扩展 | 完成 |
| 3 | 所有 E0001–E0025 规则，80% PEP 覆盖率，迁移模式 | 进行中 |
| 4 | Mojo 安全注解（所有权、不可变性、强制转换） | 计划中 |
| 5 | WASM 插件，Django/Pydantic/SQLAlchemy | 计划中 |
| 6 | 95%+ PEP，SARIF/JUnit，企业加固 | 计划中 |
| 7 | 插件市场，社区存根，生态系统 | 计划中 |

## 架构

Basilisk 是一个 Cargo 工作区，包含 14 个 Rust crate，每个拥有系统的一层：

| 层 | Crate |
|-------|--------|
| **分析管道** | `basilisk-parser` → `basilisk-resolver` → `basilisk-checker` → `basilisk-cli` |
| **LSP & 基础设施** | `basilisk-lsp`, `basilisk-db`, `basilisk-config`, `basilisk-stubs`, `basilisk-uv`, `basilisk-common` |
| **编辑器扩展** | VS Code (`vscode-extension`), Neovim (`basilisk.nvim`), Zed (`basilisk-zed`) |
| **未来** | `basilisk-mojo`（所有权），`basilisk-compiler`（原生），`basilisk-plugin`（WASM 插件） |

## 下一步

- [安装 Basilisk](/zh/docs/installation/) — 从源代码构建或通过 cargo 安装
- [快速开始](/zh/docs/quick-start/) — 5 分钟内完成第一次类型检查
- [重构](/zh/docs/refactoring/) — 所有 16 个重构代码操作
- [调试](/zh/docs/debugging/) — 设置断点，单步执行代码，检查变量
- [性能分析器](/zh/docs/profiler/) — CPU 热图，火焰图，内存泄漏检测
- [所有规则](/zh/docs/rules/) — 浏览每个 BSK-E 和 BSK-W 诊断代码
