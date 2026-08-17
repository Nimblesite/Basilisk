---
layout: layouts/blog.njk
title: "介绍 Basilisk：Python 类型系统，真正被强制执行"
description: "Python 已经有了十年的类型注解。是时候让人们默认强制执行它们了。"
date: 2026-02-28
author: Basilisk 项目
image: /assets/images/blog/introducing-basilisk.png
imageAlt: "Basilisk 类型检查流水线的抽象图像，包含验证节点与严格分析面板"
imageWidth: 1200
imageHeight: 675
tags:
  - Python typing
  - Python tooling
category: announcements
lang: zh
excerpt: "Python 类型注解生态系统有一个不可告人的秘密：没有人默认强制执行它。我们构建了 Basilisk 来解决这个问题。"
keywords: basilisk, python类型检查器, 严格类型, rust, 公告
---

Python 类型注解生态系统有一个不可告人的秘密：没有人默认强制执行它。

PEP 484 于 2015 年落地。在此后的十年里，生态系统构建了复杂的工具——Pyright、mypy、ty、Pyrefly——能够在指向完全类型化的代码时捕获真正的错误。Python 类型委员会发布了一个又一个规范。`typing` 模块发展到涵盖泛型、协议、TypeVarTuple、ParamSpec、TypeIs 等更多内容。

然而：88% 的 Python 开发人员"总是"或"经常"使用类型提示——但在这些开发人员中，近 30% 的人的 CI 管道中根本没有类型检查（[Meta/Microsoft Python 类型调查 2024](https://engineering.fb.com/2024/12/09/developer-tools/typed-python-2024-survey-meta/)）。VS Code 中最流行的类型检查器默认使用一种未类型化函数静默通过的模式。

为什么？因为每个工具都将严格性视为可选的。

## TypeScript 的教训

TypeScript 没有让 JavaScript 变得更慢或更受限制。它使大型 JavaScript 代码库变得可维护。它之所以能做到这一点，不仅仅是通过巧妙的工程，还通过一种设计理念：**类型是默认的**。您必须明确选择退出类型检查，而不是选择加入。

结果是 TypeScript 的采用遵循不同于 Python 类型采用的曲线。一旦团队使用 TypeScript，整个代码库往往都会被类型化。没有逐渐退回到未类型化代码的情况，因为工具不鼓励这样做。

Python 的类型工具采取了相反的方式。[Pyright 的四种模式](https://microsoft.github.io/pyright/#/configuration?id=type-check-diagnostics-settings)：`off`、`basic`、`standard`、`strict`。默认值不是 `strict`。大多数团队从不更改默认值。

## 其他工具的错误所在

问题不在于技术能力。Pyright 在正确配置时确实非常擅长发现类型错误。问题在于默认值。

当严格性是选择加入的时候：
- 新项目开始时没有它，因为没有立即的压力去添加它
- 现有项目从不添加它，因为第一天的错误计数令人沮丧
- CI 脚本省略了 `--strict` 标志，因为写脚本时它不在那里
- 新团队成员不知道要添加它
- 截止日期临近时该标志被丢弃

结果是一个*看起来*在使用类型检查但实际上在允许大多数类型错误静默通过的模式下运行的代码库。

## Basilisk 的立场

Basilisk 默认启用源自 PEP 的规则，没有要忘记传递的 `--strict` 标志。在撤回此前的结果后，其实际符合程度正在接受完整性审查。当你想要超出规范的检查时，只需一次配置改动：可选的 Basilisk 规则会要求每个参数都有类型、声明每个返回值，并让 `Any` 始终显式。

这不是为了让 Python 开发人员的生活更艰难。这是为了让安全路径触手可及。默认启用的是源自规范的规则集；而当团队决定想要更严格的检查时，它随时都在——在配置中开启，按项目或路径限定，绝不强加。

为现有代码库开启这种更严格的检查确实需要工作——但这是暴露真实错误的工作。开启 Basilisk 的注解规则后，每个 BSK-0001 都是一个从未定义类型契约的函数。一个非穷举的 `match` 就是一个被静默忽略的情况。这些不是误报——它们是类型系统未被使用的地方。

## 为什么选择 Rust

Basilisk 用 Rust 实现，作为单个二进制文件发布，没有运行时依赖。

替代方案——用 Python 实现 Python 类型检查器——有一个根本问题：它需要 Python 解释器才能运行。在可能运行 Docker 镜像、GitHub Actions 运行器或边缘构建系统的 CI 环境中，仅为了检查 Python 类型而添加 Python 运行时依赖是不必要的开销。

更重要的是，Rust 的所有权模型和零成本抽象使得实现 Basilisk 所需的增量计算成为可能。当您编辑单个文件时，Basilisk 只重新检查该文件及导入它的模块——即受影响的分析结果——其余的保持缓存。没有持久的守护进程，也不会在每次击键时重新分析整个项目。

结果：增量类型检查，不需要持久的守护进程，并且设计为随着代码库增长仍保持响应。

## 今天存在的内容

Basilisk (alpha) 实现了七阶段路线图的前两个阶段。

**今天可用：**
- 核心解析器、名称解析器和类型检查器
- 所有 E0001–E0025 诊断规则
- CLI：`basilisk check [path]`
- rustc 风格的错误输出，包含位置、插入符号、帮助和文档链接
- 出错时退出代码为 1，用于 CI 集成
- 递归目录检查

**同样今天可用：**
- 语言服务器协议 (LSP) 服务器——自动补全、跳转到定义、悬停、诊断、内联提示、完整的重构操作套件
- VS Code 扩展——每个平台捆绑正确的二进制文件；发布到 [Open VSX](https://open-vsx.org)（让 Cursor、Windsurf 和其他兼容 VS Code 的编辑器也可使用）即将推出
- Neovim 插件 (0.10+)
- Zed 扩展
- 集成调试器（debugpy，按 F5）
- 集成性能分析器（py-spy，火焰图，内存泄漏检测）

**路线图中：**
- 第 3 阶段：80% PEP 覆盖率，`basilisk migrate`，渐进式采用
- 第 4 阶段：WASM 插件系统，Django/Pydantic/SQLAlchemy 插件，自动存根生成
- 第 5 阶段：95%+ PEP 覆盖率，SARIF/JUnit 输出，JetBrains 扩展
- 第 6 阶段：插件市场，社区存根，生态系统

## 试用

```bash
brew tap Nimblesite/tap && brew install basilisk   # Scoop 和二进制文件：参见安装文档

git clone https://github.com/Nimblesite/Basilisk
cd Basilisk
basilisk check examples/bad.py
```

或者完全跳过 CLI——VS Code 扩展内置了该二进制文件。所有安装方式见[安装文档](/zh/docs/installation/)。

如果您想看到诊断的实际效果，存储库包含 `examples/bad.py`（每个错误都是真实的类型规范违反）、`examples/good.py`（干净的）和 `examples/mixed.py`（现实的混合情况）。

在 [GitHub](https://github.com/Nimblesite/Basilisk/issues) 上提交问题。如果您想了解完整设计，规范在 `SPEC.md` 中。

Python 类型注解已经是可选的十年了。是时候改变了。
