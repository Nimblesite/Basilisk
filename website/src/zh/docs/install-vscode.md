---
layout: layouts/docs.njk
title: 最好的 Python VS Code 扩展？安装 Basilisk
description: 在寻找最好的 Python VS Code 扩展？先看看 Basilisk 适合哪些场景，然后安装它捆绑的类型检查器、LSP、调试器、性能分析器和重构工具。
keywords: 最好的python vscode扩展, vscode的python扩展, basilisk, vs code, cursor, windsurf, open vsx, python语言服务器, 安装, vsix
lang: zh
date: 2026-02-28
dateModified: 2026-03-31
---

# VS Code、Cursor 与 Windsurf

从您编辑器的市场安装 **Basilisk** 扩展：

1. 打开您的编辑器
2. 进入扩展（`Ctrl+Shift+X` / `Cmd+Shift+X`）
3. 搜索 **Basilisk**
4. 点击**安装**

该扩展已发布到 **[VS Code 市场](https://marketplace.visualstudio.com/items?itemName=Nimblesite.basilisk)** 和 **[Open VSX](https://open-vsx.org/extension/Nimblesite/basilisk)**，因此可以安装在 **VS Code**、**Cursor**、**Windsurf** 以及其他兼容 VS Code 的编辑器中。

打开一个 Python 文件，Basilisk 会自动激活——诊断、自动补全、悬停、跳转到定义、重命名、重构、格式化、调试（F5）和性能分析。

![Basilisk 在 VS Code 中——符合 PEP 规范的类型错误以红色波浪线内联显示，并列在问题面板中](/assets/images/vscode-diagnostics.png)

*打开文件的瞬间即可获得符合 PEP 规范的诊断——无需任何配置。*

## Basilisk 是适合您的最佳 Python VS Code 扩展吗？

没有哪个 Python 扩展适合所有项目。Basilisk 面向的开发者，是希望用一个开源扩展同时获得符合类型规范的检查、自动补全、导航、重构、格式化、调试和性能分析——并且在 VS Code 之外也能使用同一个语言服务器的人。如果您的项目依赖成熟的 mypy 框架插件，或者您更偏好 Pylance 已经成型的、仅限 VS Code 的工作流，请在切换前阅读[Python 类型检查器对比](/zh/docs/comparison/)。

## 二进制文件已捆绑——无需单独安装

**扩展在 VSIX 内部附带了适合您平台的 Basilisk 二进制文件。** 默认安装无需额外设置：无需 `cargo install`，无需配置 PATH，也无需手动下载。

| 操作系统 | 架构 |
|----|-------------|
| macOS | Apple Silicon (aarch64) |
| Linux | x86_64 |
| Linux | aarch64 |
| Windows | x86_64 |
| Windows | arm64 |

## 扩展如何找到二进制文件

扩展按以下顺序解析二进制文件：

1. **显式组件路径**——`basilisk.binaries.basilisk` 或 `basilisk.executablePath`
2. **显式二进制目录**——`basilisk.binaries.path`
3. **捆绑的 VSIX 二进制文件**——`bin/<platform>/basilisk`（默认）
4. **外部安装**——Cargo、Homebrew、Scoop 或 PATH，前提是版本匹配

Homebrew 和 Scoop 是外部覆盖或修复来源。默认安装运行的是 VSIX 内捆绑的二进制文件。仅当您有意覆盖捆绑的二进制文件时——例如要运行本地构建的开发版二进制文件——才需要使用 `basilisk.executablePath`、`basilisk.binaries.basilisk` 或 `basilisk.binaries.path`。

## 后续步骤

- [快速开始](/zh/docs/quick-start/)——您的第一次类型检查
- [调试](/zh/docs/debugging/)——按 F5 开始调试
- [配置](/zh/docs/configuration/)——`pyproject.toml` 参考
