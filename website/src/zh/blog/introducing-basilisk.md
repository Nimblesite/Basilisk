---
layout: layouts/blog.njk
title: "介绍 Basilisk：Python 类型系统，真正被强制执行"
description: "Python 已经有了十年的类型注解。是时候让人们默认强制执行它们了。"
date: 2026-02-28
author: Basilisk 项目
tags: posts
category: announcements
lang: zh
excerpt: "Python 类型注解生态系统有一个不可告人的秘密：没有人默认强制执行它。我们构建了 Basilisk 来解决这个问题。"
keywords: basilisk, python类型检查器, 严格类型, rust, 公告
---

# 介绍 Basilisk：Python 类型系统，真正被强制执行

Python 类型注解生态系统有一个不可告人的秘密：没有人默认强制执行它。

PEP 484 于 2015 年落地。在此后的十年里，生态系统构建了复杂的工具——Pyright、mypy、ty、Pyrefly——能够在指向完全类型化的代码时捕获真正的错误。Python 类型委员会发布了一个又一个规范。`typing` 模块发展到涵盖泛型、协议、TypeVarTuple、ParamSpec、TypeIs 等更多内容。

然而：88% 的 Python 开发人员"总是"或"经常"使用类型提示——但在这些开发人员中，近 30% 的人的 CI 管道中根本没有类型检查（[Meta/Microsoft Python 类型调查 2024](https://engineering.fb.com/2024/12/09/developer-tools/typed-python-2024-survey-meta/)）。VS Code 中最流行的类型检查器默认使用一种未类型化函数静默通过的模式。

为什么？因为每个工具都将严格性视为可选的。

## TypeScript 的教训

TypeScript 没有让 JavaScript 变得更慢或更受限制。它使大型 JavaScript 代码库变得可维护。它之所以能做到这一点，不仅仅是通过巧妙的工程，还通过一种设计理念：**类型是默认的**。您必须明确选择退出类型检查，而不是选择加入。

结果是 TypeScript 的采用遵循不同于 Python 类型采用的曲线。一旦团队使用 TypeScript，整个代码库往往都会被类型化。没有逐渐退回到未类型化代码的情况，因为工具不鼓励这样做。

Python 的类型工具采取了相反的方式。[Pyright 的四种模式](https://microsoft.github.io/pyright/#/configuration?id=type-check-diagnostics-settings)：`off`、`basic`、`standard`、`strict`。默认值不是 `strict`。大多数团队从不更改默认值。

## 其他工具的错误所在

问题不在于技术能力。Pyright 在正确配置时，在约 99% PEP 符合性下，确实非常擅长发现类型错误。问题在于默认值。

当严格性是选择加入的时候：
- 新项目开始时没有它，因为没有立即的压力去添加它
- 现有项目从不添加它，因为第一天的错误计数令人沮丧
- CI 脚本省略了 `--strict` 标志，因为写脚本时它不在那里
- 新团队成员不知道要添加它
- 截止日期临近时该标志被丢弃

结果是一个*看起来*在使用类型检查但实际上在允许大多数类型错误静默通过的模式下运行的代码库。

## Basilisk 的立场

Basilisk 默认是严格的。没有宽松模式。没有要忘记传递的 `--strict` 标志。只有一种模式：每个参数都必须有类型，每个返回类型都必须声明，`Any` 必须始终是显式的。

这不是为了让 Python 开发人员的生活更艰难。这是为了让安全路径成为默认路径。当严格性是默认的，类型覆盖率会随着团队添加新代码而自然增加。没有什么需要记住打开的。

在现有代码库上采用 Basilisk 确实需要工作——但这是暴露真实错误的工作。每个 BSK-E0001 都是一个从未定义类型契约的函数。每个 BSK-E0040 都是调用者从未同意的变异。Basilisk 报告的错误不是误报——它们是类型系统未被使用的地方。

## Mojo 的洞察

[Mojo](https://www.modular.com/mojo) 是 Python 的超集，添加了系统编程功能：所有权语义、默认不可变性、零隐式强制转换。其函数模型在语言层面区分了 `borrowed`、`inout` 和 `owned` 参数。

Basilisk 借鉴（无双关意图）这些概念，并将其实现为对标准 Python 语法的静态分析。使用 `typing` 模块中的 `Annotated`：

```python
from typing import Annotated
from basilisk import Borrowed, InOut, Owned

def summarise(items: Annotated[list[int], Borrowed]) -> int:
    return sum(items)  # 只读——OK

def append_value(
    items: Annotated[list[int], InOut],
    value: int,
) -> None:
    items.append(value)  # 声明了变异——OK

def consume_and_sort(items: Annotated[list[int], Owned]) -> list[int]:
    items.sort()
    return items  # 所有权已转移——调用者之后不能使用 items
```

这些注解不是运行时构造。它们由 Basilisk 进行静态验证。通过这些检查的代码在结构上与 Mojo 的类型期望兼容——您的 Python 代码库可以在不等待 Mojo 编译器的情况下变得 Mojo 就绪。

## 为什么选择 Rust

Basilisk 用 Rust 实现，作为单个二进制文件发布，没有运行时依赖。

替代方案——用 Python 实现 Python 类型检查器——有一个根本问题：它需要 Python 解释器才能运行。在可能运行 Docker 镜像、GitHub Actions 运行器或边缘构建系统的 CI 环境中，仅为了检查 Python 类型而添加 Python 运行时依赖是不必要的开销。

更重要的是，Rust 的所有权模型和零成本抽象使得实现 Basilisk 所需的增量计算成为可能。我们使用 [Salsa](https://github.com/salsa-rs/salsa)——驱动 rust-analyzer 的相同增量计算框架——实现亚 10 毫秒的增量类型检查。当您编辑单个文件时，Basilisk 只重新计算受影响的分析结果。其余的保持缓存。

结果：响应击键的类型检查，不需要持久的守护进程，在大型代码库上不消耗千兆字节内存，也不会随着代码库增长而变慢。

## 今天存在的内容

Basilisk v0.1.0 实现了七阶段路线图的第 1 阶段。

**今天可用：**
- 核心解析器、名称解析器和类型检查器
- 所有 E0001–E0025 诊断规则
- CLI：`basilisk check [path]`
- rustc 风格的错误输出，包含位置、插入符号、帮助和文档链接
- 出错时退出代码为 1，用于 CI 集成
- 递归目录检查

**积极开发中：**
- 语言服务器协议 (LSP) 服务器
- VS Code 扩展
- Neovim 配置

**路线图中：**
- 第 3 阶段：80% PEP 覆盖率，`basilisk migrate`，迁移模式
- 第 4 阶段：Mojo 安全注解（所有权、不可变性、强制转换）
- 第 5 阶段：WASM 插件系统，Django/Pydantic/SQLAlchemy 插件，自动存根生成
- 第 6 阶段：95%+ PEP 覆盖率，SARIF/JUnit 输出，企业加固
- 第 7 阶段：插件市场，社区存根，生态系统

## 试用

```bash
git clone https://github.com/MelbourneDeveloper/Basilisk
cd basilisk
cargo build --release
./target/release/basilisk check examples/bad.py
```

如果您想看到诊断的实际效果，存储库包含 `examples/bad.py`（含有意的错误）、`examples/good.py`（干净的）和 `examples/mixed.py`（现实的混合情况）。

在 [GitHub](https://github.com/MelbourneDeveloper/Basilisk/issues) 上提交问题。如果您想了解完整设计，规范在 `SPEC.md` 中。

Python 类型注解已经是可选的十年了。是时候改变了。
