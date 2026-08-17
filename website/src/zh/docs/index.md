---
layout: layouts/docs.njk
title: 简介
description: 安装、配置并使用 Basilisk：集类型检查、语言服务、重构、调试、性能分析和编辑器集成于一体的开源工具。
keywords: basilisk, python, 语言服务器, lsp, 类型检查器, vs code, cursor, zed, neovim, 严格, rust
lang: zh
---

# 简介

Basilisk 是一个**完整的开源 Python 语言服务器**。您依赖现代 Python 扩展提供的一切——自动补全、跳转到定义、悬停信息、重构、诊断、集成调试、性能分析——Basilisk 全部提供，完全开源。其默认规则旨在实现 Python 类型规范，而该实现目前正在接受完整性审查。

**符合性更正：**Basilisk 此前的结果已撤回，当前百分比暂时未知，并已应我们的请求从官方 `python/typing` 结果中移除。我们正在逐条审计规则，并删除那些只匹配源文本、并未做真正类型检查的规则。数字会先下降；只有在通过稳健性测试和变异验证后，才会发布新结果。请阅读[完整更正](/zh/docs/conformance/)。

它不仅仅是一个类型检查器。它是一个功能完整的 LSP，已为 **VS Code**、**Cursor**、**Windsurf**、**Zed** 和 **Neovim** 提供扩展——以及支持语言服务器协议的其他编辑器。JetBrains（IntelliJ / PyCharm）支持已纳入计划。无专有扩展。无 Node.js。单个 Rust 二进制文件，在每款编辑器中提供相同的体验。

## Basilisk 解决的问题

[Pylance](https://marketplace.visualstudio.com/items?itemName=ms-python.vscode-pylance) 是 VS Code 中默认的 Python 语言扩展。它也是**专有软件**——您无法检查、修改或重新发布它。Pyright，底层的开源类型检查器，很强大，但它*只是*一个类型检查器——没有专有 Pylance 包装，它不提供补全、悬停、跳转到定义或重构。

其他每个 Python 类型检查器（mypy、ty、Pyrefly）都*只是*检查器——没有补全、没有重构、没有调试器。你得另外搭一个语言服务器，并让两者在团队中保持同步。

Basilisk 采取不同的立场。它默认启用类型规范规则，并将整个工具栈（类型检查、语言功能、调试、性能分析）整合为一个开源工具，在**每一款**编辑器中运行方式相同，而不仅仅是 VS Code。默认规则的实际符合程度暂时未知，正在接受审计；凡是依据源文本判断的规则都会被删除。想要比规范更严格的检查？开启可选的 Basilisk 规则。

## Basilisk 是什么

- **功能完整的语言服务器** (LSP)——自动补全、跳转到定义、悬停、查找引用、重命名、[完整的重构套件](/zh/docs/refactoring/)、代码操作、内联提示
- **覆盖主流编辑器的扩展**——VS Code、Cursor、Windsurf、Neovim (0.10+) 和 Zed 现已支持；JetBrains（IntelliJ / PyCharm）已纳入计划
- **注解快速修复**——一键代码操作，为未注解的代码插入占位注解（`: Any`、`-> None`），方便您填入真实类型
- **集成调试器**——按 F5 调试 Python，支持断点、单步执行、变量检查和监视表达式，全部通过 Basilisk LSP 代理
- **集成性能分析器**——采样式 CPU 分析器，具有内联热图注解、火焰图、内存泄漏检测和引用图可视化，全部在您的编辑器内
- **默认启用 PEP 规则的类型检查器**——开箱即启用核心规范规则集，并提供可选的 Basilisk 规则以实现比规范更严格的检查
- **开箱即得标准库类型**——二进制文件中编译进了一份完整的 typeshed `stdlib/` 树，且检查从不下载任何东西，因此无需网络、无需配置即可获得标准库类型；固定某个确切的 `python/typeshed` 提交后，它会离线对照本地存储库校验
- **用于 CI 集成的 CLI 工具**——发现错误时以代码 1 退出
- **迁移助手**，读取您现有的 `pyrightconfig.json` 或 `mypy.ini`
- **uv 集成**——工作区检测、锁文件解析和包管理命令
- 用 **Rust** 编写——作为单个二进制文件发布，没有运行时依赖

## Basilisk 不是什么

- 不是编译器——您的 Python 代码照常在 CPython 上运行
- 不是运行时类型检查器——分析在开发时静态发生
- 不依赖特定编辑器——同一个服务器驱动 VS Code、Cursor、Windsurf、Zed 和 Neovim

## 默认启用规范规则，并可从此配置

Basilisk 的行为完全由**配置**决定，而默认配置启用**核心 PEP 规则集**——与官方类型符合性套件评分所用的规则相同。这些规则旨在无需额外严格模式即可遵循规范，但在撤回旧结果后，其实际符合程度暂时未知，正在接受审计；凡是依据源文本判断的规则都会被删除。

比规范更严格的检查是**可选启用**的。Basilisk 还附带规范未定义的额外规则——要求每个参数和返回值都有注解、冗余注解警告、缺失 `@override` 提示、显式 `Any` 提示。在你于配置中启用之前，它们始终**关闭**。由于它们会标记规范视为有效的代码，刻意开启它们就是用严格的规范符合换取由团队自行选择的更严格标准——这是逐项目的选择，绝非默认。

配置同样是你为需要的路径放宽规则的地方——在那个文件夹放置带 `[tool.basilisk]` 表的 `pyproject.toml`，最近作出决定的表对其下的文件胜出：

```toml
# legacy/pyproject.toml
[tool.basilisk.rules]
"returns_compatibility" = "warning"   # 仅为遗留代码降级
"imports_unresolved" = "info"
```

这让默认设置专注于源自规范的规则，同时让每个团队在需要的地方精确调节额外严格程度。

## 项目状态

Basilisk 正在**积极开发中**——核心检查器、LSP 服务器和编辑器扩展都在工作。此前的符合性结果已撤回，检查器规则正在接受审计，只匹配源文本的规则会被删除。自动补全、跳转到定义、悬停、诊断、内联提示、重构、调试和性能分析今天就在发布。

| 阶段 | 里程碑 | 状态 |
|---|---|---|
| 1 | 解析器、解析器、类型检查器、CLI | 完成 |
| 2 | LSP 服务器、编辑器扩展（VS Code、Cursor、Zed、Neovim） | 完成 |
| 3 | 审计并删除依据文本判断的规则、建立保持语义的变异测试、渐进式采用 | 进行中 |
| 4 | WASM 插件，Django/Pydantic/SQLAlchemy | 计划中 |
| 5 | SARIF/JUnit 输出，JetBrains 扩展 | 计划中 |
| 6 | 插件市场，社区存根，生态系统 | 计划中 |

## 架构

Basilisk 是一个 Cargo 工作区，包含 18 个 Rust crate，每个拥有系统的一层：

| 层 | Crate |
|-------|--------|
| **分析管道** | `basilisk-parser` &rarr; `basilisk-resolver` &rarr; `basilisk-checker` &rarr; `basilisk-cli` |
| **LSP & 基础设施** | `basilisk-lsp`, `basilisk-db`, `basilisk-config`, `basilisk-stubs`, `basilisk-uv`, `basilisk-common`, `basilisk-buildinfo`, `basilisk-profiler-helper`, `basilisk-profiler-protocol` |
| **Typeshed 下载** | `basilisk-typeshed-fetch` — 工作区中唯一的 HTTP 客户端；仅在用户显式操作时下载 typeshed，与类型检查严格隔离 |
| **测试基础设施** | `basilisk-test-utils`, `basilisk-test-macros` |
| **编辑器扩展** | VS Code (`vscode-extension`), Neovim (`basilisk.nvim`), Zed (`basilisk-zed`) |

## 下一步

- [安装 Basilisk](/zh/docs/installation/) — Homebrew、Scoop、编辑器市场或从源代码构建
- [快速开始](/zh/docs/quick-start/) — 5 分钟内完成第一次类型检查
- [重构](/zh/docs/refactoring/) — 完整的重构套件（提取、内联、移动、重命名、转换）
- [调试](/zh/docs/debugging/) — 设置断点，单步执行代码，检查变量
- [性能分析器](/zh/docs/profiler/) — CPU 热图，火焰图，内存泄漏检测，引用图
- [所有规则](/zh/docs/rules/) — 浏览每个 BSK-E 和 BSK-W 诊断代码
