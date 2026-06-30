---
layout: layouts/docs.njk
title: Python 类型检查工具对比
description: "Basilisk 与 Pyright、mypy、ty、Pyrefly 等 Python 类型检查工具的对比：严格性、PEP 符合性、性能与功能。"
keywords: basilisk vs pyright, python 类型检查工具对比, mypy vs basilisk, ty, pyrefly
lang: zh
---

# Python 类型检查工具对比

Python 类型检查器的格局已经发生了重大变化。2025 年推出了三个基于 Rust 的新工具。它们的差异与其说在于原始速度，不如说在于对类型规范的实现有多忠实——以及它究竟是一个完整的语言服务器，还是仅仅一个检查器。

## 根本问题

在比较功能和性能之前，有一个问题决定了你究竟能否信任某个检查器的判断：

**它究竟实现了官方类型规范的多少？**

| 工具 | PEP 符合性（官方套件¹） |
|---|---|
| Basilisk | {{ conformance.scorePct }}%（{{ conformance.pass }}/{{ conformance.total }}） |
| Pyright | ~99% |
| Pyrefly | ~86% |
| mypy | ~58% 完全通过 |
| ty | 早期 alpha |

不实现某个规范特性的检查器，就无法判断使用该特性的代码——它要么漏掉真实错误，要么凭空制造误报。Basilisk 的**默认**规则集*就是*类型规范：它只运行核心 PEP 符合性规则，别无其他，并在我们固定的提交上通过套件中的每一个文件。规则选择完全由配置驱动，因此默认就是核心 PEP 规则集——绝不更多。

想要比规范更严格的检查？在配置中开启**可选的 Basilisk 规则**。它们默认关闭，并且按设计会标记规范*不*视为错误的东西（比如未注解的参数）——所以开启它们实际上会*破坏*对规范的严格符合。这正是要点：当你的团队想要超出规范的检查时再启用它们，而不是强加给每个项目。

---

## 完整功能比较

| 功能 | Basilisk | Pyright | mypy | ty | Pyrefly |
|---|---|---|---|---|---|
| 补全修复（自动添加类型） | ✅ | ❌ | ❌ | ❌ | ❌ |
| 超出规范的可选规则 | ✅ 配置 | `--strict` | `--strict` | ❌ | ❌ |
| PEP 符合性¹ | {{ conformance.scorePct }}%（{{ conformance.pass }}/{{ conformance.total }}） | ~99% | ~58% | 早期 alpha | ~86% |
| 实现语言 | Rust | TypeScript | Python/C | Rust | Rust |
| 需要运行时 | 无 | Node.js | Python | 无 | 无 |
| 完整 LSP（补全、悬停、跳转） | ✅ | 仅 Pylance | ❌ | 基础 | 基础 |
| 集成调试器 | ✅ | ❌ | ❌ | ❌ | ❌ |
| 集成性能分析器 | ✅ | ❌ | ❌ | ❌ | ❌ |
| 编辑器扩展 | VS Code、Zed、Neovim（Cursor/Windsurf 的 Open VSX 即将推出） | 专有（Pylance） | 无 | VS Code | VS Code |
| 插件系统 | WASM（计划中） | 无 | Python 钩子 | 计划中 | 无 |
| 许可证 | MIT | MIT | MIT | MIT | MIT |

<a name="footnotes"></a>

**来源：**

¹ 完全通过得分来自[官方 python/typing 符合性套件](https://github.com/python/typing/blob/main/conformance/results/results.html)（pyright 1.1.408、mypy 1.19.1、pyrefly 0.54.0）。mypy 的部分+通过得分为 96.4%。ty 尚未包含在官方套件中——alpha 阶段的数据来自 [sinon.github.io/future-python-type-checkers](https://sinon.github.io/future-python-type-checkers/)（2025 年 8 月，alpha 版本）。

---

## Pyright

**由微软开发。基于 TypeScript。约 99% PEP 符合性（[来源](https://github.com/python/typing/blob/main/conformance/results/results.html)）。**

Pyright 是目前可用的最符合 PEP 的 Python 类型检查器。它正确处理了绝大多数 PEP 类型功能，对于基于 TypeScript 的工具来说性能出色。

**Pyright 做得好的地方：**
- 在任何现有工具中最高的 PEP 覆盖率（官方符合性套件约 99%）
- 出色的文档和错误消息
- 通过 Pylance 深度集成 VS Code
- 在大多数代码库中足够快用于交互使用
- 对复杂泛型和协议的良好推断

**Pyright 不做的事情：**
- 无集成调试器或性能分析器——它检查类型，但不是完整的开发环境
- 需要 Node.js 运行——为仅 Python 的 CI 环境添加依赖
- Pylance（VS Code 扩展）是专有的——其最丰富的功能不离开 VS Code
- 无插件——无法添加框架特定的类型智能

**Pyright 何时有意义：** 如果您已经深度投入微软的 VS Code 生态系统并且不介意 Node.js 依赖，Pyright 当前的 PEP 符合性使其成为当今纯类型检查的最强选择。Basilisk 的目标是在第 3 阶段超过其符合性。

---

## mypy

**原创。基于 Python/C。约 58% 完全通过，96% 部分+通过（[来源](https://github.com/python/typing/blob/main/conformance/results/results.html)）。**

mypy 定义了 Python 类型检查的样子。多年来，其 `--strict` 标志是 Python 类型中"严格"含义的参考实现。

**mypy 做得好的地方：**
- 已建立的插件生态系统：Django、SQLAlchemy、Pydantic 都有 mypy 插件
- `--strict` 标志有充分的文档记录和理解
- 最大的社区和最多的 StackOverflow 答案
- 悠久的历史意味着处理了大多数边缘情况

**mypy 不做的事情：**
- 在大型代码库上比基于 Rust 的工具慢得多
- 守护进程模式（`dmypy`）在某些条件下不稳定
- 不是语言服务器——没有补全、悬停或跳转到定义
- 需要 Python 运行时
- 插件 API 仅 Python——无 WASM 可移植性

**mypy 何时有意义：** 在 mypy 插件（Django、SQLAlchemy）上有大量投资的现有代码库可能会发现迁移工作很重大，直到 Basilisk 的 WASM 插件生态系统达到同等水平。

---

## ty（Astral）

**由 Ruff 团队构建。Rust + Salsa。早期 alpha——尚未在官方符合性套件中。**

ty 是最有趣的新入场者。它由创建 Ruff 的同一团队构建（现在是事实上的 Python linter），使用基于 Salsa 的增量架构，与 Basilisk 一样用 Rust 构建，并拥有 Astral 的工程速度支持。

**ty 做得好的地方：**
- 基于 Rust 的增量架构（Salsa）
- 由有交付记录的团队构建
- MIT 许可，完全开源
- 亚 10 毫秒的增量速度（[PyTorch 上 4.7ms](https://astral.sh/blog/ty)，2025 年 12 月）

**ty 尚不做的事情：**
- 尚未包含在[官方 python/typing 符合性套件](https://github.com/python/typing/blob/main/conformance/results/results.html)中——仍处于早期 alpha
- 默认渐进类型
- 无集成调试器或性能分析器

**ty 何时有意义：** 如果您愿意押注 Astral 的开发速度并能容忍采用期间较低的类型覆盖率。ty 最终可能成为主要参与者；现在依赖它进行严格执行还为时过早。

---

## Pyrefly（Meta）

**在 Instagram 规模上经过生产测试。基于 Rust。约 86% PEP 符合性（[来源](https://github.com/python/typing/blob/main/conformance/results/results.html)）。**

Pyrefly 由 Meta 构建，用于处理他们的 Python 代码库——世界上最大的代码库之一。它强调吞吐量（[1.85M LOC/秒，166 核 Meta 基础设施](https://pyrefly.org/)）而不是严格执行。

**Pyrefly 做得好的地方：**
- 在数百万行生产 Python 上经过实战测试
- 适合单仓库规模代码库的高吞吐量
- 基于 Rust，无运行时依赖
- 良好的文档

**Pyrefly 不做的事情：**
- 无集成调试器或性能分析器
- 无插件系统
- Meta 驱动的路线图——外部贡献影响较少

**Pyrefly 何时有意义：** 超大型代码库（50 万行以上），吞吐量比严格执行更重要，尤其是如果团队有与 Meta 相关的工具。

---

## Basilisk 的定位

Basilisk 不是现有工具的更快版本。它占据了不同的位置：

**Basilisk 独有的：**
1. 开箱即 100% PEP 符合（基于我们固定的套件提交），并提供**可选的 Basilisk 规则**，可在配置中开启以获得比规范更严格的检查——除非你主动启用，它们从不运行，也从不影响符合性得分
2. 补全修复——一键代码操作，自动添加缺失的类型，而不仅仅是报告缺少
3. 在每款编辑器中完整的开源 LSP——补全、悬停、跳转到定义、重构、调试和性能分析，在 VS Code 以及原生 Zed 和 Neovim 扩展中相同（Cursor、Windsurf 等的 Open VSX 即将推出；JetBrains 计划中）——不仅仅在一个专有的 VS Code 扩展内
4. 通过语言服务器代理的集成调试器和性能分析器
5. WASM 插件系统（计划中）——无需分叉即可扩展，设计安全

**Basilisk 尚不是最佳选择的地方：**
- 成熟度与边缘情况广度：Basilisk 在我们[固定的提交](/zh/docs/conformance/)上通过官方套件的 {{ conformance.scorePct }}%（{{ conformance.pass }}/{{ conformance.total }}，错误*和*警告，最严格评分），{{ conformance.fp }} 处误报、{{ conformance.missed }} 处遗漏的必需错误。这衡量的是对规范的符合，而非多年的打磨：Pyright 在套件之外仍能处理更多真实世界的边缘情况，Pylance 今天功能完整。Basilisk 处于 alpha 阶段。
- 插件生态系统：mypy 的 Django 和 SQLAlchemy 插件已经成熟。Basilisk 的 WASM 插件是计划中的。
- 成熟度：Pylance 今天功能完整（虽然是专有的，且仅限 VS Code）。Basilisk 处于 alpha 阶段。

诚实的建议：开始新 Python 项目的团队从第一天起就能用 Basilisk 获得完全符合 PEP 的检查——并可在准备好时开启比规范更严格的规则——尤其是如果他们跨多款编辑器工作。在大型、良好类型化代码库上从 Pyright 迁移的团队仍应权衡 Pyright 的成熟度与 Pylance 的功能完整性。
