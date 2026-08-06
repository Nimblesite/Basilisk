---
layout: layouts/docs.njk
title: Python 类型检查工具对比
description: "Basilisk 与 Pyright、mypy、ty、Pyrefly 等 Python 类型检查工具的对比：严格性、PEP 符合性、性能与功能。"
keywords: basilisk vs pyright, python 类型检查工具对比, mypy vs basilisk, ty, pyrefly
lang: zh
dateModified: 2026-08-06
---

# Python 类型检查工具对比

Python 类型检查器的格局已经发生了重大变化。它们的差异在于对类型规范的实现有多忠实、究竟是一个完整的语言服务器还是仅仅一个检查器，以及速度。Basilisk 之前公开的性能数据目前已[撤回并等待审查](/docs/benchmarks/)。

<p class="bench-caveat"><strong>符合性更正：</strong>Basilisk 此前的结果已撤回，当前百分比暂时未知。应我们的请求，Basilisk 已从<a href="https://github.com/python/typing/blob/main/conformance/results/results.html">官方结果表</a>中移除，同时受影响的逻辑正在重新实现并接受独立稳健性验证。请勿使用旧得分或旧排名比较这些工具。</p>

## 根本问题

在比较功能和性能之前，有一个问题决定了你究竟能否信任某个检查器的判断：

**除了用于衡量它的固定测试之外，它究竟实现了官方类型规范的多少？**

对于当前列出的检查器，请查看[官方实时结果表](https://github.com/python/typing/blob/main/conformance/results/results.html)。Basilisk 目前不在表中。旧结果无法通过保持语义不变的测试变异，因此在替换受影响实现期间，Basilisk 的诚实答案暂时是**未知**。详情见[符合性更正](/zh/docs/conformance/)。

想要比规范更严格的检查？在配置中开启**可选的 Basilisk 规则**。它们默认关闭，并且按设计会标记规范*不*视为错误的东西（比如未注解的参数）, 所以开启它们实际上会*破坏*对规范的严格符合。这正是要点：当你的团队想要超出规范的检查时再启用它们，而不是强加给每个项目。

---

## 完整功能比较

下表中每一个对勾、叉号和标签都链接到支撑它的权威来源（官方文档、代码仓库或 LICENSE）。当某个工具的能力超出一行所能容纳时，脚注会加以说明。

| 功能 | Basilisk | Pyright | mypy | ty | Pyrefly |
|---|---|---|---|---|---|
| 注解快速修复（插入占位符） | ✅ `: Any` / `-> None` ² | ❌ ³ | ❌ ⁴ | 双击内联提示 ⁵ | ❌（代码操作） |
| 自动插入*推断*类型 | ❌ | ❌ ³ | ❌ ⁴ | ❌ | ✅ CLI `pyrefly infer` ⁶ |
| 超出规范的可选规则 | ✅ 配置 | strict 模式 ⁷ | `--strict` ⁴ | 仅严重级别 ⁸ | ✅ `strict` 预设 ⁹ |
| PEP 符合性¹ | **暂时未知；旧结果已撤回** | 见实时结果 | 见实时结果 | 见实时结果 | 见实时结果 |
| 实现语言 | Rust | TypeScript ³ | Python/C ⁴ | Rust ¹⁰ | Rust ¹¹ |
| 需要运行时 | 无 | Node.js ³ | Python ⁴ | 无 ¹⁰ | 无 ¹¹ |
| 补全、悬停、跳转 | ✅ | ✅ ¹² | ❌ ⁴ | ✅ ¹³ | ✅ ¹⁴ |
| 集成调试器 | ✅ | ❌ ³ | ❌ ⁴ | ❌ ¹³ | ❌ ¹⁴ |
| 集成性能分析器 | ✅ | ❌ ³ | ❌ ⁴ | ❌ ¹³ | ❌ ¹⁴ |
| 编辑器扩展 | VS Code、Zed、Neovim（Cursor/Windsurf 的 Open VSX 即将推出） | VS Code（开源 + 专有 Pylance）¹⁵ | 无官方 ¹⁶ | VS Code、PyCharm、Neovim、Zed ¹⁷ | VS Code + 多款（经 LSP）¹⁴ |
| 插件系统 | WASM（计划中） | 无 ³ | Python 钩子 ¹⁸ | 无 ¹⁹ | 无 ⁹ |
| 许可证 | MIT | MIT ²⁰（Pylance 专有 ¹⁵） | MIT ²¹ | MIT ²² | MIT ²³ |

<a name="footnotes"></a>

**来源：**

¹ 当前列出的检查器请参见[官方 python/typing 实时结果](https://github.com/python/typing/blob/main/conformance/results/results.html)。Basilisk 在撤回旧结果后请求移除；只有在全新实现通过稳健性和变异验证后，才会发布当前百分比。

² Basilisk 的快速修复插入的是**占位符**注解（参数和属性为 `: Any`，返回值为 `-> None`；空集合变量为 `list[Any]` / `dict[str, Any]`），供你替换为真实类型。它不推断类型。参见[缺失注解规则](/zh/docs/rules/missing-annotations/)。

³ [Pyright 文档](https://microsoft.github.io/pyright/#/features) 与[源码](https://github.com/microsoft/pyright)：Pyright 唯一的源码快速修复是"创建类型存根"；它用 TypeScript 编写，需要 [Node.js](https://microsoft.github.io/pyright/#/installation)，且[没有插件机制](https://microsoft.github.io/pyright/#/configuration)。它仅是类型检查器，没有调试器或性能分析器。

⁴ [mypy 文档](https://mypy.readthedocs.io/en/stable/)：mypy [用 Python 编写并由 mypyc 编译](https://mypyc.readthedocs.io/en/latest/introduction.html)，需要 Python 运行时，有 [`--strict`](https://mypy.readthedocs.io/en/stable/command_line.html) 标志，且不是语言服务器（[`dmypy` 守护进程](https://mypy.readthedocs.io/en/stable/mypy_daemon.html) 加速检查，而非补全/悬停/跳转）。它可经 `dmypy suggest` 输出草稿签名，但由单独的工具（PyAnnotate）写入，因此 mypy 本身不插入注解。

⁵ [ty 语言服务器文档](https://docs.astral.sh/ty/features/language-server/)：ty 没有自动添加注解的代码操作，但其内联提示"可双击以将类型注解插入源码"。

⁶ [Pyrefly `autotype` 文档](https://pyrefly.org/en/docs/autotype/)：`pyrefly infer` 将**推断出的**参数、返回值和容器注解直接写入源码（CLI，积极开发中）。这超出了 Basilisk 的占位符修复，因此该行归功于 Pyrefly，而非我们。

⁷ [Pyright 配置](https://microsoft.github.io/pyright/#/configuration)：严格检查通过 `typeCheckingMode: "strict"`（配置项或 `# pyright: strict`）启用，而非 `--strict` CLI 标志。

⁸ [ty 规则](https://docs.astral.sh/ty/rules/)：ty 允许更改规则**严重级别**（`--error all` 提升现有规范规则），但未记录任何超出规范的严格预设。

⁹ [Pyrefly 配置](https://pyrefly.org/en/docs/configuration/)：`strict` 预设启用额外检查（`implicit-any`、`missing-override-decorator` 等），任何错误代码都可单独启用。Pyrefly 未记录任何插件系统。

¹⁰ [ty 代码仓库](https://github.com/astral-sh/ty) 与[安装](https://docs.astral.sh/ty/installation/)：用 Rust（Salsa）编写，作为独立二进制文件发布，无需 Node.js/Python 运行时。

¹¹ [Pyrefly 代码仓库](https://github.com/facebook/pyrefly) 与[安装](https://pyrefly.org/en/docs/installation/)：用 Rust 编写，作为独立二进制文件发布，无运行时依赖。

¹² [Pyright 功能](https://microsoft.github.io/pyright/#/features)：开源（MIT）pyright 语言服务器提供补全、悬停和跳转到定义；专有的 [Pylance](https://github.com/microsoft/pylance-release/blob/main/FAQ.md) 在其之上增加语义高亮、重构和 IntelliCode。

¹³ [ty 语言服务器](https://docs.astral.sh/ty/features/language-server/)：实现补全、悬停、跳转、引用、重命名、签名帮助、代码操作等（格式化委托给 Ruff）。无调试器或性能分析器。

¹⁴ [Pyrefly IDE 文档](https://pyrefly.org/en/docs/IDE/)：功能齐全的语言服务器（悬停、补全、定义、引用、重命名、代码操作、调用层次等），带第一方 VS Code/Open VSX 扩展，并记录了通过 LSP 在 Neovim、Vim、Emacs、JetBrains、Zed、Helix、Sublime 和 Jupyter 中的配置。无调试器或性能分析器。

¹⁵ [Pyright README](https://github.com/microsoft/pyright)：发布开源（MIT）VS Code 扩展；微软更丰富的默认体验 [Pylance](https://marketplace.visualstudio.com/items?itemName=ms-python.vscode-pylance) 是[专有的](https://github.com/microsoft/pylance-release/blob/main/FAQ.md)。

¹⁶ mypy 项目不发布第一方编辑器扩展；微软维护的第三方扩展 [`ms-python.mypy-type-checker`](https://marketplace.visualstudio.com/items?itemName=ms-python.mypy-type-checker) 仅提供诊断。

¹⁷ [ty 编辑器文档](https://docs.astral.sh/ty/editors/)：官方/一流支持 VS Code（Astral 维护的扩展）、PyCharm（2025.3+）、Neovim 和 Zed，以及任何经 `ty server` 的 LSP 编辑器。

¹⁸ [扩展 mypy](https://mypy.readthedocs.io/en/stable/extending_mypy.html)：mypy 有 Python 插件 API（子类化 `mypy.plugin.Plugin`），被 Django、SQLAlchemy 和 Pydantic 插件使用。

¹⁹ ty 的[文档](https://docs.astral.sh/ty/) 和[发布公告](https://astral.sh/blog/ty) 未记录任何插件系统，也未宣布计划中的插件系统。

²⁰ [Pyright LICENSE.txt](https://github.com/microsoft/pyright/blob/main/LICENSE.txt), MIT。

²¹ [mypy LICENSE](https://github.com/python/mypy/blob/master/LICENSE), MIT。

²² [ty LICENSE](https://github.com/astral-sh/ty/blob/main/LICENSE), MIT。

²³ [Pyrefly LICENSE](https://github.com/facebook/pyrefly/blob/main/LICENSE), MIT。

---

## Pyright

**由微软开发，基于 TypeScript。当前符合性请参见[官方结果](https://github.com/python/typing/blob/main/conformance/results/results.html)。**

Pyright 长期是符合性的领跑者，至今仍是最强的检查器之一。它处理广泛的 PEP 类型功能，并拥有成熟的编辑器生态系统。

**Pyright 做得好的地方：**
- 强大的 PEP 覆盖率；请参见官方实时符合性结果
- 出色的文档和错误消息
- 通过 Pylance 深度集成 VS Code
- 在大多数代码库中足够快用于交互使用
- 对复杂泛型和协议的良好推断

**Pyright 不做的事情：**
- 无集成调试器或性能分析器：它检查类型，但不是完整的开发环境
- 需要 Node.js 运行，为仅 Python 的 CI 环境添加依赖
- Pylance（VS Code 扩展）是专有的：其最丰富的功能不离开 VS Code
- 无插件，无法添加框架特定的类型智能

**Pyright 何时有意义：** 如果您已经深度投入微软的 VS Code 生态系统并且不介意 Node.js 依赖，Pyright 仍是一个强大、成熟的选择。

---

## mypy

**原创，基于 Python/C。当前符合性请参见[官方结果](https://github.com/python/typing/blob/main/conformance/results/results.html)。**

mypy 定义了 Python 类型检查的样子。多年来，其 `--strict` 标志是 Python 类型中"严格"含义的参考实现。

**mypy 做得好的地方：**
- 已建立的插件生态系统：Django、SQLAlchemy、Pydantic 都有 mypy 插件
- `--strict` 标志有充分的文档记录和理解
- 最大的社区和最多的 StackOverflow 答案
- 悠久的历史意味着处理了大多数边缘情况

**mypy 不做的事情：**
- 检查需要 Python 运行时
- 守护进程模式（`dmypy`）在某些条件下不稳定
- 不是语言服务器，没有补全、悬停或跳转到定义
- 需要 Python 运行时
- 插件 API 仅 Python，无 WASM 可移植性

**mypy 何时有意义：** 在 mypy 插件（Django、SQLAlchemy）上有大量投资的现有代码库可能会发现迁移工作很重大，直到 Basilisk 的 WASM 插件生态系统达到同等水平。

---

## ty（Astral）

**由 Ruff 团队构建，使用 Rust + Salsa。当前符合性请参见[官方结果](https://github.com/python/typing/blob/main/conformance/results/results.html)。**

ty 是最有趣的新入场者。它由创建 Ruff 的同一团队构建（现在是事实上的 Python linter），使用基于 Salsa 的增量架构，与 Basilisk 一样用 Rust 构建，并拥有 Astral 的工程速度支持。

**ty 做得好的地方：**
- 基于 Rust 的增量架构（Salsa）
- 由有交付记录的团队构建
- MIT 许可，完全开源
- 亚 10 毫秒的增量速度（[PyTorch 上 4.7ms](https://astral.sh/blog/ty)，2025 年 12 月）

**ty 尚不做的事情：**
- 类型实现仍在成熟中
- 默认渐进类型
- 无集成调试器或性能分析器

**ty 何时有意义：** 如果您重视 Astral 的工具生态，并愿意采用一个快速发展的检查器。

---

## Pyrefly（Meta）

**在 Instagram 规模上经过生产测试，基于 Rust。当前符合性请参见[官方结果](https://github.com/python/typing/blob/main/conformance/results/results.html)。**

Pyrefly 由 Meta 构建，用于处理他们的 Python 代码库，世界上最大的代码库之一。它强调吞吐量（[1.85M LOC/秒，166 核 Meta 基础设施](https://pyrefly.org/)）而不是严格执行。

**Pyrefly 做得好的地方：**
- 在数百万行生产 Python 上经过实战测试
- 适合单仓库规模代码库的高吞吐量
- 基于 Rust，无运行时依赖
- 良好的文档

**Pyrefly 不做的事情：**
- 无集成调试器或性能分析器
- 无插件系统
- Meta 驱动的路线图，外部贡献影响较少

**Pyrefly 何时有意义：** 超大型代码库（50 万行以上），吞吐量比严格执行更重要，尤其是如果团队有与 Meta 相关的工具。

---

## Basilisk 的定位

Basilisk 不是现有工具的更快版本。它占据了不同的位置：

**Basilisk 的组合：**
1. 默认启用类型规范规则，并提供**可选的 Basilisk 规则**以实现比规范更严格的检查。符合性实现正在重新构建，当前百分比暂时未知
2. 注解快速修复，一键代码操作，为未注解的代码插入占位符注解（`: Any`、`-> None`），方便你填入真实类型，而不用手动找位置
3. 在每款编辑器中完整的开源 LSP, 补全、悬停、跳转到定义、重构、调试和性能分析，在 VS Code 以及原生 Zed 和 Neovim 扩展中相同（Cursor、Windsurf 等的 Open VSX 即将推出；JetBrains 计划中）, 不仅仅在一个专有的 VS Code 扩展内
4. 通过语言服务器代理的集成调试器和性能分析器
5. WASM 插件系统（计划中）, 无需分叉即可扩展，设计安全

**Basilisk 仍在成长的地方：**
- Basilisk 正在积极开发中。此前的符合性结果已撤回；受影响逻辑正在从头实现，[当前百分比暂时未知](/zh/docs/conformance/)。
- 插件生态系统：mypy 的 Django 和 SQLAlchemy 插件已经成熟。Basilisk 的 WASM 插件是计划中的。

建议：根据 Basilisk 集成的开源编辑器工作流进行评估，并在您自己的代码上测试它。不要依据已撤回的符合性或基准测试数据做出选择。待全新实现和稳健性审查完成后，我们会发布新的符合性结果。
