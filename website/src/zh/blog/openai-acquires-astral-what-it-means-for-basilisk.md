---
layout: layouts/blog.njk
title: "OpenAI 收购 Astral：这对 Basilisk 意味着什么"
description: "OpenAI 正在收购 Astral——Ruff、uv 和 ty 的缔造者。Basilisk 构建于 Ruff 之上。本文逐条说明什么会变、什么不会变，以及我们对保持开源的承诺。"
date: 2026-06-20
author: Basilisk 项目
tags: posts
category: announcements
lang: zh
excerpt: "Basilisk 的解析器就是 Ruff。Ruff 的所有者即将变成 OpenAI。本文将逐句、以一手资料为依据说明这意味着什么——以及为什么你的 Basilisk 安装无论如何都是安全的。"
keywords: basilisk, astral, openai, ruff, uv, ty, charlie marsh, python类型检查器, 开源, fork, MIT 许可证
---

**2026 年 3 月 19 日**，Astral——Ruff、uv 与 ty 类型检查器背后的公司——宣布已"达成协议，作为 Codex 团队的一部分加入 OpenAI"（[astral.sh/blog/openai](https://astral.sh/blog/openai)；[openai.com/index/openai-to-acquire-astral](https://openai.com/index/openai-to-acquire-astral/)）。

Basilisk 直接构建于 Astral 的成果之上。所以这对我们而言并非抽象的行业新闻——它触及产品的根基。本文将说明究竟发生了什么、当事人到底说了什么、它会改变 Basilisk 的什么、又不会改变什么。下文的每一条主张都链接到其来源。凡是重要的引述，我们都在刊出前与第二个来源进行了交叉核对。而在合理的人有所担忧之处，我们也照样引用了这些担忧——准确比唱赞歌更重要。

## 究竟发生了什么

- OpenAI 正在收购 **Astral**，即开源 Python 工具 **Ruff**（代码检查器/格式化工具）、**uv**（包/项目管理器）与 **ty**（类型检查器）的缔造者。交易完成后，Astral 团队将加入 OpenAI 的 **Codex** 团队（[openai.com](https://openai.com/index/openai-to-acquire-astral/)，[astral.sh](https://astral.sh/blog/openai)）。
- 交易**尚未完成**。它"仍需满足惯常的交割条件，包括监管批准"，且"在此之前，OpenAI 与 Astral 将保持各自独立的公司身份"（OpenAI 公告，[openai.com/index/openai-to-acquire-astral](https://openai.com/index/openai-to-acquire-astral/)）。
- OpenAI 对交割后的意图表态："交割后，OpenAI 计划继续支持 Astral 的开源产品"，并将"随着时间推移，探索更深度的集成，让 Codex 能够更直接地与开发者已经在使用的工具交互"（[openai.com/index/openai-to-acquire-astral](https://openai.com/index/openai-to-acquire-astral/)）。
- OpenAI 对**原因**的阐述："我们对 Codex 的目标，是超越只会生成代码的 AI，迈向能够参与整个开发工作流的系统……Astral 的开发者工具正处在这条工作流之中"（OpenAI，引自 [The Register](https://www.theregister.com/2026/03/19/openai_aims_for_the_stars/)）。或者说得更直白："通过把 Astral 的工具与工程能力带入 OpenAI，我们将加速 Codex 的工作"（OpenAI，经 [Simon Willison](https://simonwillison.net/2026/mar/19/openai-acquiring-astral/) 引述）。

## 该给的赞誉要给：Charlie Marsh 的公开表态

Charlie Marsh 用大约三年时间，把 Ruff 和 uv 打造成了现代 Python 工具链事实上的标准。这是开发者工具史上最精彩的成就之一，值得大声说出来。以下是他在宣布交易时所写（[astral.sh/blog/openai](https://astral.sh/blog/openai)）：

> "我创立 Astral，是为了让编程更具生产力。"
>
> "今天，我们朝着这一使命迈进了一步：宣布我们已达成协议，作为 Codex 团队的一部分加入 OpenAI。"

关于开源——这是对 Astral 下游所有人最要紧的一句话，也是我们与第二个来源独立核实过的一句：

> "开源是这份影响力的核心，也是这个故事的核心；它处在我们所做一切的中心。"
>
> "交易完成后，OpenAI 将继续支持我们的开源工具。我们会继续公开地构建，与我们的社区同行。"

这就是承诺，出自创始人本人之口。该给的赞誉要给：Marsh 没有含糊其辞，而 OpenAI 自己的公告也用它自己的声音说了同样的话（见上）。**向 Charlie Marsh 致敬**——Basilisk 之所以能存在，部分正是因为他的团队以宽松许可证向世界提供了一个世界级的 Python 解析器，我们不打算让这一点被忽视。

## "更多开源，而非更少"：创始人加码表态

宣布当天的一篇博文是一回事。Marsh 几周后即兴说出的话更能说明问题。在 **Talk Python To Me 第 552 期**（"Astral joins OpenAI"，录制于 2026 年 6 月 2 日）中，主持人 **Michael Kennedy** 在节目简介里把社区的本能反应摆上了台面："如果你的第一反应是'等等，uv 是不是要完了？'，你并不孤单。但 Charlie Marsh 跟我分享了一个反转：他认为他们在 OpenAI 可能会比在 Astral 时做出更多开源"（[talkpython.fm 第 552 期](https://talkpython.fm/episodes/show/552/astral-joins-openai)）。

Marsh 本人在该期节目中的原话：

> "我真心认为，我们在这里最终写出的开源，有可能比我们在 Astral 时还要多。"

而且，关键在于，他给出了自己希望被评判的标准——不是承诺，而是过往的实绩：

> "你没法只靠言辞说服所有人。你真正要做的，是用日积月累的行动去说服人们。"

> "如果你喜欢我们做过的工作，也对我们运营仓库和社区的方式感到满意，那么我想你会对接下来的发展感到满意。"

我们认同他立下的这条标准：**以行动评判，以时间为尺。** 这篇文章，就是我们公开地、带着凭据，开始为这个时钟计时。

## 基石：许可证（经核实，而非假定）

口头保证是好的。**许可证才有约束力。** 这是整个故事中最重要的事实，因此我们没有依赖任何摘要，而是直接对照源代码仓库逐一核实：

| 工具 | 许可证 | 来源 |
|---|---|---|
| **Ruff** | MIT——"MIT License. Copyright (c) 2022 Charles Marsh" | [astral-sh/ruff/LICENSE](https://github.com/astral-sh/ruff/blob/main/LICENSE) |
| **uv** | 双许可："可任选 Apache License 2.0 版或 MIT 许可证之一" | [astral-sh/uv](https://github.com/astral-sh/uv#license) |
| **ty** | MIT——"ty 以 MIT 许可证授权" | [astral-sh/ty](https://github.com/astral-sh/ty#license) |

宽松许可证对已经发布的代码而言是不可撤销的。任何收购方——OpenAI 也好，任何人也罢——都无法追溯地收回今天已存在的 Ruff 的授权。授权早已给出。未来所有者最多只能对**未来的**提交更改许可证，到那一步，社区会保留最后一个开源提交并从那里继续。正如 Astral 的 **Douglas Creager** 所说，宽松许可证"让最坏情形的形状变成了'分叉，然后继续前进'"（经 [Simon Willison](https://simonwillison.net/2026/mar/19/openai-acquiring-astral/) 引述）。在 Lobsters 上，法律层面的机制被说得同样直白："uv 采用 MIT 与 Apache 2.0 双许可，两者都允许再授权"（[lobste.rs](https://lobste.rs/s/dhogio/thoughts_on_openai_acquiring_astral_uv)）。

## 生态系统里的其他人怎么看

做这道算术题的下游项目不止我们一家。

- **Armin Ronacher**（Flask 作者）称 uv 是"一个非常容易分叉、也非常容易维护的东西"，并认为即便在最坏情形下，社区也会比 uv 出现之前过得更好（经 [Simon Willison](https://simonwillison.net/2026/mar/19/openai-acquiring-astral/) 引述；JetBrains 在[他们的文章](https://blog.jetbrains.com/pycharm/2026/03/openai-acquires-astral-what-it-means-for-pycharm-users/)里也呼应了"非常容易分叉、容易维护"这一说法）。
- **Simon Willison**（Datasette 作者）："我喜欢也信任 Astral 团队，我对他们的项目在新家会得到良好维护持乐观态度"（[simonwillison.net](https://simonwillison.net/2026/mar/19/openai-acquiring-astral/)）。
- **JetBrains**（PyCharm）作为 Ruff 与 uv 的直接集成方表示："我们之所以把 Ruff 和 uv 都集成进 PyCharm，是因为它们实实在在地让 Python 开发更好了……我们会继续这样做。"他们还补充了对所有下游都至关重要的一点："Astral 的工具以宽松许可证开源。如果真到了那一步，社区可以分叉它们。"以及"无论这些工具归谁所有，我们为用户支持最好的 Python 工具的承诺都不会改变"（[blog.jetbrains.com](https://blog.jetbrains.com/pycharm/2026/03/openai-acquires-astral-what-it-means-for-pycharm-users/)）。
- **Lobsters** 讨论里有一个被低估、却反复出现的观点：其中很多软件在功能上其实已经做完了。"我多少把 uv/ruff 之类看作'已完成'的软件，就算它们明天起不再有任何提交，它们依然有用"（[lobste.rs](https://lobste.rs/s/dhogio/thoughts_on_openai_acquiring_astral_uv)）。分叉并不是从零开始；它是从一个成熟、久经实战检验的代码库开始的。

## 质疑者值得被认真对待

只摆出令人安心的引述是不诚实的。严肃的人提出了严肃的担忧，而且他们有理由这么做。

- **被当作筹码的风险。** 总体乐观的 Simon Willison 仍然精准地点出了失败模式："这桩交易的一个糟糕版本会是：OpenAI 开始把对 `uv` 的所有权当作与 Anthropic 竞争中的筹码"（[simonwillison.net](https://simonwillison.net/2026/mar/19/openai-acquiring-astral/)，[The Register](https://www.theregister.com/2026/03/19/openai_aims_for_the_stars/) 也引用了这句）。基础性工具沦为 AI 实验室之间的竞争武器，是一个真实的——尽管尚未发生的——危险。
- **治理上的空白。** 这桩交易在战略上比在治理托管上要清晰。正如 [The New Stack](https://thenewstack.io/openai-astral-acquisition/) 在标题里所概括的，OpenAI 把这些工具带入了 Codex，"——但细节仍然模糊"。目前并没有关于 uv、Ruff 或 ty 作为社区项目的长期治理方案的公开说明。
- **"本来就是 VC 资助"的批评。** 早在这桩交易之前，一些开发者就对依赖风险投资支持的工具感到不安，而在他们看来，这次收购正是那条路可预见的终点。这种情绪贯穿于 [Python Discourse 讨论帖](https://discuss.python.org/t/openai-to-acquire-astral/106605) 与 [Lobsters](https://lobste.rs/s/dhogio/thoughts_on_openai_acquiring_astral_uv)。这是一个值得权衡的、合理的哲学层面的反对意见——也正是我们让 Basilisk 的根基保持锁定且可分叉、而不去信任任何单一厂商路线图的原因之一。

诚实的总结是：**许可证**让最坏情形能够幸存；这些项目的**开放治理**才是真正悬而未决的问题。而这恰恰就是 Marsh 请大家来评判他的那一点——"以日积月累的行动"。

## 这对 Basilisk 具体意味着什么

Basilisk 与 Astral 的关系是具体而承重的：

1. **我们的解析器就是 Ruff 的解析器。** Basilisk 依赖 `ruff_python_parser`、`ruff_python_ast` 与 `ruff_text_size`，并将它们钉在 `astral-sh/ruff` 上一个**不可变的 git 提交**（`rev 7c645a9`，等同于标签 `0.15.17`）。我们钉的是 `rev` 而非标签，正是为了让这个版本"永远无法被人从我们脚下换掉"。那份代码是 MIT 授权的，并且已经写进了我们的 `Cargo.lock`。这次收购无法回过头去改变我们所构建的那些字节。
2. **我们的检查/格式化路径调用的是 Ruff CLI**——`ruff==0.15.17`，在 CI 和开发容器中钉得完全一致。同样的道理：一个我们自己掌控版本的、宽松许可的二进制文件。
3. **ty 如今是有 OpenAI 撑腰的竞争对手。** Astral 的类型检查器 ty 与 Basilisk 的检查器处在同一概念空间，如今它背后将有 OpenAI 的资源。我们认真对待这一点——但它磨砺、而非威胁了 Basilisk 的差异化所在：**默认严格的合规性**、集成在单一扩展中的一套**完整 LSP**（测试浏览器、调试、性能分析、自动修复），以及朝着 100% PEP 合规一路推进的执着。一个资金更雄厚的类型检查器，恰恰印证了"Python 值得拥有一流、Rust 级速度的工具"这一判断——它并不会让我们的差异化变得不再成立。
4. **共同的架构押注——如今得到了印证。** 与 Astral 的工具一样，Basilisk 用 Rust 构建、基于 Ruff AST、以 Salsa 实现增量计算。Astral 已经证明这套技术栈能扩展到数以百万计的用户。我们独立地做出了同样的选择。这令人安心，而非令人不安。

**对今天的你而言，净影响：** 零。你的 Basilisk 安装构建自被钉死的、MIT 授权的 Ruff 代码和一个被钉死的 Ruff 二进制文件。这次收购不会、也不能改变其中任何一个。

## Nimblesite 的承诺

我们要把话说清楚，因为我们的用户应当得到一个直截了当的答案：

**Nimblesite 承诺让 Basilisk 的根基保持开源——并且，如果有必要，我们会分叉 Ruff。**

MIT 许可证让这件事不仅可行，而且干净利落。我们已经把 Ruff 钉在一个不可变的提交上，这在设计上就是一个随时待命的分叉：如果上游的方向、许可证或托管方式有朝一日不再服务于 Basilisk 的用户，我们会依据其现有的开源条款，亲自把这个解析器继续推进下去。而且正如上文社区所指出的，这份代码已经足够成熟，分叉所继承的是一个扎实的基础，而非一个半成品。我们希望永远不必如此。但若真有那一天，我们已做好万全准备。能够说走就走的自由，正是宽松许可证为你买来的东西——也正是我们当初选择构建于 Ruff 之上的原因。

## 给 OpenAI 的鼓励之词

我们想以正确的方式收尾，因为这里的信号确实是好的，而好的行为值得被点名称赞。

OpenAI 本可以对开源只字不提。它选择了表态——在它自己的公告中（"交割后，OpenAI 计划继续支持 Astral 的开源产品"，[openai.com](https://openai.com/index/openai-to-acquire-astral/)），也通过 Charlie Marsh，而他在公开场合说得更进一步："我真心认为，我们在这里最终写出的开源，有可能比我们在 Astral 时还要多"（[Talk Python 第 552 期](https://talkpython.fm/episodes/show/552/astral-joins-openai)）。这些都是一家公司可以被追责的承诺，而我们打算替整个生态系统记住它们。

致 OpenAI：**谢谢你们让这些工具保持开放，也请让它们一直如此。** Ruff 和 uv 之所以成为标准，正**因为**它们开放、快速、由社区共建。慷慨地托管它们——把改进回馈上游、让解析器保持可作为独立 crate 复用、如承诺般公开构建、并抵御住 Simon Willison 所点出的把它们当作筹码的诱惑——这不是慈善；这正是让这项资产值得被收购的原因本身。守住对许可证和社区的信义，你们就会得到我们的信任。Basilisk 真心希望你们把这件事做对，而我们将成为众多下游项目中的一个，日复一日地、安静地证明：为什么开放的根基值得守护。

---

*完整来源：[OpenAI 公告](https://openai.com/index/openai-to-acquire-astral/) · [Astral 公告（Charlie Marsh）](https://astral.sh/blog/openai) · [Talk Python To Me 第 552 期（Charlie Marsh 与 Michael Kennedy）](https://talkpython.fm/episodes/show/552/astral-joins-openai) · [JetBrains / PyCharm](https://blog.jetbrains.com/pycharm/2026/03/openai-acquires-astral-what-it-means-for-pycharm-users/) · [Simon Willison](https://simonwillison.net/2026/mar/19/openai-acquiring-astral/) · [The Register](https://www.theregister.com/2026/03/19/openai_aims_for_the_stars/) · [The New Stack](https://thenewstack.io/openai-astral-acquisition/) · [Python Discourse 讨论帖](https://discuss.python.org/t/openai-to-acquire-astral/106605) · [Lobsters 讨论帖](https://lobste.rs/s/dhogio/thoughts_on_openai_acquiring_astral_uv) · 许可证：[Ruff (MIT)](https://github.com/astral-sh/ruff/blob/main/LICENSE)，[uv (Apache-2.0 / MIT)](https://github.com/astral-sh/uv#license)，[ty (MIT)](https://github.com/astral-sh/ty#license)。*
