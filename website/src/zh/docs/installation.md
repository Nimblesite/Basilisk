---
layout: layouts/docs.njk
title: 安装 Basilisk——VS Code、Cursor、Zed、Neovim 或 CLI
description: 为您的编辑器安装 Basilisk Python 语言服务器——VS Code、Cursor、Windsurf、Zed 或 Neovim——或作为独立 CLI 通过 PyPI（uv tool install 或 pipx）、Homebrew、Scoop 或预构建二进制文件安装。单个 Rust 二进制文件，无运行时依赖。
keywords: basilisk, 安装, 最好的python类型检查器, vs code, cursor, windsurf, zed, neovim, pypi, pip, uv, pipx, homebrew, scoop, open vsx, python语言服务器, rust
lang: zh
date: 2026-02-28
dateModified: 2026-07-19
---

# 安装

Basilisk 是一个单一的 Rust 二进制文件，没有运行时依赖——无需 Node.js，无需 Python 解释器，安装后也不需要包管理器。**在每一款受支持的编辑器中，二进制文件都随扩展一同提供；您永远不需要单独安装它。**

选择您的环境：

| 如果您使用…… | 安装指南 | 二进制文件…… |
|---|---|---|
| **VS Code、Cursor、Windsurf** | [VS Code 与 Cursor](/zh/docs/install-vscode/) | 捆绑在扩展内部 |
| **Zed** | [Zed](/zh/docs/install-zed/) | 首次运行时随扩展下载 |
| **Neovim** | [Neovim](/zh/docs/install-neovim/) | 首次使用时由插件下载 |
| **命令行 / CI** | [CLI 与包管理器](/zh/docs/install-cli/) | 通过 PyPI（`uv tool install`）、Homebrew、Scoop 或发布二进制文件安装 |

## 编辑器支持（LSP）

Basilisk 实现了语言服务器协议，因此任何支持 LSP 的编辑器都可以使用它：

- **VS Code**——官方扩展，捆绑二进制文件 → [指南](/zh/docs/install-vscode/)
- **Cursor、Windsurf 及其他 VS Code 分支**——通过 [Open VSX](https://open-vsx.org) → [指南](/zh/docs/install-vscode/)
- **Zed**——原生扩展，自动下载二进制文件 → [指南](/zh/docs/install-zed/)
- **Neovim**——官方 `basilisk.nvim` 插件，自动下载二进制文件 → [指南](/zh/docs/install-neovim/)
- **Helix**——原生 LSP 支持（将其指向一个 [CLI 安装](/zh/docs/install-cli/)）
- **Emacs**——通过 eglot 或 lsp-mode（将其指向一个 [CLI 安装](/zh/docs/install-cli/)）
- **JetBrains（IntelliJ / PyCharm）**——即将支持

## 各编辑器的集成状态

完整的 Basilisk 工作流在各编辑器中的现状——✅ 已交付，🌗 部分支持，⛔️ 尚未支持：

| IDE                            | 用户估算 | 已发布 | LSP | 格式化 | 性能分析 | 内存 | 调试 | 测试 | MCP |
|--------------------------------|:----------:|:--------:|:---:|:------:|:---------:|:------:|:---------:|:-------:|:---:|
| VS Code                        | [5000 万月活](https://developer.microsoft.com/blog/celebrating-50-million-developers-the-journey-of-visual-studio-and-visual-studio-code) |    ✅    | ✅  |   ✅   |    ✅     |   ✅   |    ✅     |   ✅    | ⛔️ |
| IntelliJ / PyCharm             | [1140 万](https://www.jetbrains.com/lp/annualreport-2024/) |    ⛔️    | ⛔️ |   🌗   |    ⛔️     |   ⛔️   |    ⛔️     |   ⛔️    | ⛔️ |
| OpenVSX（Cursor、Windsurf 等） | [100 万+ 日活](https://cursor.com/blog/series-d) |    ✅    | ✅  |   ✅   |    ✅     |   ✅   |    ✅     |   ✅    | ⛔️ |
| Emacs                          | [约 100 万](https://en.wikipedia.org/wiki/Emacs) |    ⛔️    | ⛔️ |   🌗   |    ⛔️     |   ⛔️   |    ⛔️     |   ⛔️    | ⛔️ |
| Vim                            | [约占命令行用户的 1/3](https://en.wikipedia.org/wiki/Vim_(text_editor)) |    ⛔️    | ⛔️ |   🌗   |    ⛔️     |   ⛔️   |    ⛔️     |   ⛔️    | ⛔️ |
| Sublime Text                   | [约 1.5% 市场份额](https://6sense.com/tech/ides-and-text-editors/sublime-text-market-share) |    ⛔️    | ⛔️ |   🌗   |    ⛔️     |   ⛔️   |    ⛔️     |   ⛔️    | ⛔️ |
| Zed                            | [每日数十万](https://en.wikipedia.org/wiki/Zed_(text_editor)) |    ✅    | ⛔️  |   🌗   |    ✅     |   ✅   |    ✅     |   ✅    | ⛔️ |
| Neovim                         | [约 18 万](https://en.wikipedia.org/wiki/Neovim) |    🌗    | ✅  |   ⛔️   |    🌗     |   ✅   |    ✅     |   🌗    | ⛔️ |

用户估算均链接到其出处，取自各平台自行公布的数字，而非我们的测量结果。

## 后续步骤

安装完成后，请前往[快速开始](/zh/docs/quick-start/)运行您的第一次类型检查，或查看[配置参考](/zh/docs/configuration/)在 `pyproject.toml` 中调整 Basilisk。
