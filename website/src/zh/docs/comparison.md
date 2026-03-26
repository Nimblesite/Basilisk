---
layout: layouts/docs.njk
title: 类型检查器对比
description: Basilisk 与 Pyright、mypy、ty 和 Pyrefly 在严格性、性能和功能方面的比较。
keywords: basilisk vs pyright, python类型检查器比较, mypy vs basilisk, ty, pyrefly
lang: zh
eleventyNavigation:
  key: Comparison
  order: 6
---

# 类型检查器对比

Python 类型检查器的格局已经发生了重大变化。2025 年推出了三个基于 Rust 的新工具。它们每一个都默认使用渐进类型。

## 根本问题

在比较功能和性能之前，有一个问题决定了类型检查器是否真的能在团队中强制执行类型安全：

**该工具是否默认标记未类型化的代码？**

| 工具 | 默认标记未类型化代码？ |
|---|---|
| Basilisk | 是 |
| Pyright | 否——必须传递 `--strict` 或配置 `typeCheckingMode = "strict"` |
| mypy | 否——必须传递 `--strict` |
| ty | 否 |
| Pyrefly | 否 |

除 Basilisk 外的每个工具都允许未类型化的代码在其默认配置中静默通过。当严格性是选择加入的时候，它往往不会发生。处于截止日期压力下的团队跳过标志。新项目从不添加它。CI 脚本省略它。

Basilisk 消除了这个选择。没有宽松模式可以回退。

---

## 完整功能比较

| 功能 | Basilisk | Pyright | mypy | ty | Pyrefly |
|---|---|---|---|---|---|
| 默认严格 | ✅ | ❌ 选择加入 | ❌ 选择加入 | ❌ 选择加入 | ❌ 选择加入 |
| PEP 符合性 | 100% 目标 | ~99% | ~58% | 早期 alpha | ~86% |
| 实现语言 | Rust | TypeScript | Python/C | Rust | Rust |
| 需要运行时 | 无 | Node.js | Python | 无 | 无 |
| 增量速度 | <10ms | ~386ms | 更慢 | 4.7ms | <10ms |
| 所有权分析 | ✅ | ❌ | ❌ | ❌ | ❌ |
| 不可变性强制执行 | ✅ | ❌ | ❌ | ❌ | ❌ |
| 强制转换检测 | ✅ | ❌ | ❌ | ❌ | ❌ |
| 插件系统 | WASM | 无 | Python 钩子 | 计划中 | 无 |
| VS Code 扩展 | 开源 | 专有 (Pylance) | 无 | 开源 | 开源 |
| Mojo 兼容性 | ✅ | ❌ | ❌ | ❌ | ❌ |
| SARIF 输出 | ✅（第 3 阶段） | ✅ | ❌ | ❌ | ✅ |
| 许可证 | MIT | MIT | MIT | MIT | MIT |

---

## Pyright

**由微软开发。基于 TypeScript。约 99% PEP 符合性。**

Pyright 是目前可用的最符合 PEP 的 Python 类型检查器。它正确处理了绝大多数 PEP 类型功能，对于基于 TypeScript 的工具来说性能出色。

**Pyright 做得好的地方：**
- 在任何现有工具中最高的 PEP 覆盖率（官方符合性套件约 99%）
- 出色的文档和错误消息
- 通过 Pylance 深度集成 VS Code
- 在大多数代码库中足够快用于交互使用
- 对复杂泛型和协议的良好推断

**Pyright 不做的事情：**
- 默认不严格——四种模式：`off`、`basic`、`standard`、`strict`
- 需要 Node.js 运行——为仅 Python 的 CI 环境添加依赖
- Pylance（VS Code 扩展）是专有的——并非所有功能在 VS Code 之外都可用
- 无所有权或不可变性分析
- 无插件——无法添加框架特定的类型智能
- 无强制转换检测

**Pyright 何时有意义：** 如果您已经投资于微软的 VS Code 生态系统并且不介意 Node.js 依赖，Pyright 当前的 PEP 符合性使其成为当今纯类型检查的最强选择。Basilisk 的目标是在第 3 阶段超过其符合性。

---

## mypy

**原创。基于 Python/C。约 58% 完全通过，96% 部分通过。**

mypy 定义了 Python 类型检查的样子。多年来，其 `--strict` 标志是 Python 类型中"严格"含义的参考实现。

**mypy 做得好的地方：**
- 已建立的插件生态系统：Django、SQLAlchemy、Pydantic 都有 mypy 插件
- `--strict` 标志有充分的文档记录和理解
- 最大的社区和最多的 StackOverflow 答案
- 悠久的历史意味着处理了大多数边缘情况

**mypy 不做的事情：**
- 在大型代码库上比基于 Rust 的工具慢得多
- 守护进程模式（`dmypy`）在某些条件下不稳定
- 默认不严格
- 需要 Python 运行时
- 无所有权分析，无强制转换检测
- 插件 API 仅 Python——无 WASM 可移植性

**mypy 何时有意义：** 在 mypy 插件（Django、SQLAlchemy）上有大量投资的现有代码库可能会发现迁移工作很重大，直到 Basilisk 的 WASM 插件生态系统达到同等水平。

---

## ty（Astral）

**由 Ruff 团队构建。Rust + Salsa。早期 alpha——尚未在官方符合性套件中。**

ty 是最有趣的新入场者。它由创建 Ruff 的同一团队构建（现在是事实上的 Python linter），使用与 Basilisk 相同的基于 Salsa 的增量架构，并拥有 Astral 的工程速度支持。

**ty 做得好的地方：**
- 与 Basilisk 相同的架构基础（Salsa + Rust）
- 由有交付记录的团队构建
- MIT 许可，完全开源
- 亚 10 毫秒的增量速度

**ty 尚不做的事情：**
- 尚未包含在官方 python/typing 符合性套件中——仍处于早期 alpha
- 默认渐进类型
- 无所有权分析

---

## Pyrefly（Meta）

**在 Instagram 规模上经过生产测试。基于 Rust。约 86% PEP 符合性。**

Pyrefly 由 Meta 构建，用于处理他们的 Python 代码库——世界上最大的代码库之一。它强调吞吐量而不是严格执行。

**Pyrefly 做得好的地方：**
- 在数百万行生产 Python 上经过实战测试
- 适合单仓库规模代码库的高吞吐量
- 基于 Rust，无运行时依赖
- 良好的文档

**Pyrefly 不做的事情：**
- 默认不严格——不可用
- 无所有权或不可变性分析
- 无插件系统
- Meta 驱动的路线图——外部贡献影响较少

---

## Basilisk 的定位

Basilisk 不是现有工具的更快版本。它占据了不同的位置：

**Basilisk 独有的：**
1. 默认严格——唯一不能意外在宽松模式下运行的工具
2. 所有权分析——`Borrowed`、`InOut`、`Owned` 语义，经过静态验证
3. 不可变性强制执行——参数是只读的，除非另有声明
4. 强制转换检测——隐式 `int`→`float`、`bool`→`int`、`bytes`→`str` 是类型错误
5. WASM 插件系统——无需分叉即可扩展，设计安全
6. Mojo 兼容性——通过 Basilisk 检查的代码在结构上已准备好用于 Mojo

**Basilisk 尚不是最佳选择的地方：**
- PEP 符合性：第 1 阶段实现 E0001–E0025。Pyright 今天覆盖更多边缘情况。Basilisk 的目标是 100%；还未达到。
- 插件生态系统：mypy 的 Django 和 SQLAlchemy 插件已经成熟。Basilisk 的 WASM 插件是第 5 阶段。
- VS Code 扩展：Basilisk 扩展是第 2 阶段。Pylance 今天功能完整（虽然是专有的）。

诚实的建议：开始新 Python 项目的团队应该使用 Basilisk，从第一天起就受益于严格执行。在现有良好类型化代码库上从 Pyright 迁移的团队应该在第 3 阶段评估，届时覆盖率达到同等水平。
