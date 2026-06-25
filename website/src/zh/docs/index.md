---
layout: layouts/docs.njk
title: 简介
description: Basilisk 是完整的开源 Python 语言服务器——默认严格的类型检查、自动补全、重构、调试和性能分析，支持 VS Code、Cursor、Windsurf、Zed 和 Neovim。
keywords: basilisk, python, 语言服务器, lsp, 类型检查器, vs code, cursor, zed, neovim, 严格, rust
lang: zh
---

# 简介

Basilisk 是一个**完整的开源 Python 语言服务器**。您依赖现代 Python 扩展提供的一切——自动补全、跳转到定义、悬停信息、重构、诊断、集成调试、性能分析——Basilisk 全部提供，完全开源，默认严格模式。

它不仅仅是一个类型检查器。它是一个功能完整的 LSP，为 **VS Code**、**Zed** 和 **Neovim** 提供一流扩展——以及任何支持语言服务器协议的其他编辑器。**Cursor** 和 **Windsurf**（通过 Open VSX）即将推出，JetBrains（IntelliJ / PyCharm）也在路上。无专有扩展。无 Node.js。单个 Rust 二进制文件，在每款编辑器中提供相同的体验。

## Basilisk 解决的问题

[Pylance](https://marketplace.visualstudio.com/items?itemName=ms-python.vscode-pylance) 是 VS Code 中默认的 Python 语言扩展。它也是**专有软件**——您无法检查、修改或重新发布它。Pyright，底层的开源类型检查器，很强大，但它*只是*一个类型检查器——没有专有 Pylance 包装，它不提供补全、悬停、跳转到定义或重构。

其他每个 Python 类型检查器（mypy、ty、Pyrefly）都默认使用*渐进类型*。未类型化的代码静默通过。`Any` 在类型推断中扩散而不发出警告。严格性是您必须故意选择加入、配置、记住在 CI 中强制执行，并向每个新团队成员重新解释的东西。

Basilisk 采取不同的立场。它将整个工具栈——类型检查、语言功能、调试和性能分析——整合为一个默认严格的开源工具，在**每一款**编辑器中运行方式相同，而不仅仅是 VS Code。类型注解是契约，不是文档。

## Basilisk 是什么

- **功能完整的语言服务器** (LSP)——自动补全、跳转到定义、悬停、查找引用、重命名、[完整的重构套件](/zh/docs/refactoring/)、代码操作、内联提示
- **适配每款主流编辑器的扩展**——VS Code、Neovim (0.10+) 和 Zed 现已支持；Cursor 和 Windsurf（通过 Open VSX）即将推出，JetBrains（IntelliJ / PyCharm）也在路上
- **补全修复**——一键代码操作，自动为您添加缺失的类型注解
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
- 不依赖特定编辑器——同一个服务器驱动 VS Code、Cursor、Windsurf、Zed 和 Neovim

## 只有一种模式

Basilisk 有一种单一的操作模式。没有 `--basic`、`--standard` 或 `--permissive` 标志。这是故意的。

当严格性是选择加入的时候，团队会向宽松的默认值漂移。截止日期来临。技术债务积累。`--strict` 标志从未被添加到 CI 脚本中。Basilisk 完全消除了这种可能性。

选择退出仍然是可能的——对于遗留目录，您可以按路径禁用或降低特定规则：

```toml
[tool.basilisk.per-path-overrides."legacy/**"]
disabled = ["BSK-E0011"]        # 为遗留代码完全禁用某规则
rules."BSK-E0010" = "warning"   # 或仅降低其严重性
```

这承认大型代码库不能在一夜之间完全类型化，同时使放宽保持明确并限定在需要的路径上。

## 项目状态

Basilisk 目前处于 **alpha**——核心检查器、LSP 服务器和编辑器扩展都在工作。自动补全、跳转到定义、悬停、诊断、内联提示、重构、调试和性能分析今天就在发布。

| 阶段 | 里程碑 | 状态 |
|---|---|---|
| 1 | 解析器、解析器、类型检查器、CLI | 完成 |
| 2 | LSP 服务器、编辑器扩展（VS Code、Cursor、Zed、Neovim） | 完成 |
| 3 | 扩展规则集，PEP 符合性攻坚（当前 {{ conformance.scorePct }}%，目标 100%），渐进式采用 | 进行中 |
| 4 | 所有权与不可变性分析（Mojo 启发） | 计划中 |
| 5 | WASM 插件，Django/Pydantic/SQLAlchemy | 计划中 |
| 6 | 95%+ PEP，SARIF/JUnit，JetBrains 扩展 | 计划中 |
| 7 | 插件市场，社区存根，生态系统 | 计划中 |

## 架构

Basilisk 是一个 Cargo 工作区，包含 16 个 Rust crate，每个拥有系统的一层：

| 层 | Crate |
|-------|--------|
| **分析管道** | `basilisk-parser` &rarr; `basilisk-resolver` &rarr; `basilisk-checker` &rarr; `basilisk-cli` |
| **LSP & 基础设施** | `basilisk-lsp`, `basilisk-db`, `basilisk-config`, `basilisk-stubs`, `basilisk-uv`, `basilisk-common`, `basilisk-test-utils`, `basilisk-profiler-helper` |
| **编辑器扩展** | VS Code (`vscode-extension`), Neovim (`basilisk.nvim`), Zed (`basilisk-zed`) |
| **未来** | `basilisk-mojo`（所有权），`basilisk-compiler`（原生），`basilisk-plugin`（WASM 插件） |

## 下一步

- [安装 Basilisk](/zh/docs/installation/) — Homebrew、Scoop、编辑器市场或从源代码构建
- [快速开始](/zh/docs/quick-start/) — 5 分钟内完成第一次类型检查
- [重构](/zh/docs/refactoring/) — 完整的重构套件（提取、内联、移动、重命名、转换）
- [调试](/zh/docs/debugging/) — 设置断点，单步执行代码，检查变量
- [性能分析器](/zh/docs/profiler/) — CPU 热图，火焰图，内存泄漏检测，引用图
- [所有规则](/zh/docs/rules/) — 浏览每个 BSK-E 和 BSK-W 诊断代码
