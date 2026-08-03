---
layout: layouts/blog.njk
title: "Basilisk 在 Python 类型符合性测试套件上达到 100%"
description: "Basilisk 现已进入官方 python/typing 符合性结果，取得完美的 100%，是唯一达到这一成绩的 Python 类型检查器。本文解释这意味着什么。"
date: 2026-07-11
author: Christian Findlay
image: /assets/images/blog/basilisk-100-conformance.png
imageAlt: "Python 类型检查器符合性排行榜，显示 Basilisk 取得完美的 100% 分数"
imageWidth: 1200
imageHeight: 675
tags:
  - Python typing
category: announcements
lang: zh
excerpt: "本周 Basilisk 加入了官方 python/typing 符合性结果，并取得了完美的分数。它是排行榜上唯一达到 100% 的 Python 类型检查器。本文解释这个数字究竟意味着什么、榜上还有谁，以及我们为什么不会过度宣传它。"
keywords: python类型检查器, python类型符合性, python/typing符合性结果, basilisk, mypy, pyright, ty, pyrefly, zuban, pep符合性, 严格类型
faq:
  - q: "哪个 Python 类型检查器的符合性分数最高？"
    a: "在官方 python/typing 符合性结果中，Basilisk 0.27.0 取得完美的 100%（141 项测试中通过 141 项）。它是榜上唯一达到 100% 的检查器。zuban、Pyrefly 和 Pyright 紧随其后，均在 96% 以上。竞品分数会随着这些工具的改进而变化，因此请查看每个工具的实时结果文件夹以获取当前数字。"
  - q: "什么是 python/typing 符合性测试套件？"
    a: "这是由 Python Typing 社区维护的官方测试套件，用于衡量类型检查器对 Python 类型规范的实现有多忠实。每个检查器都针对同一组测试运行，并由该套件自己的评分工具评分。结果发布在 github.com/python/typing 的 conformance/results 下。"
  - q: "100% 的符合性分数是否等于是最好的类型检查器？"
    a: "不。python/typing 的维护者明确表示，符合性不应成为选择类型检查器的主要依据，因为它无法反映速度、编辑器集成、错误信息质量或生态系统支持。符合性只衡量规范正确性。它是一个重要的输入，但不是全部决定因素。"
  - q: "Basilisk 的符合性分数是如何测量的？"
    a: "Basilisk 由 python/typing 套件自己未经修改的评分工具评分，针对默认配置的 Basilisk CLI 运行，开启所有规范规则。没有内置的自制评分器，也没有特殊配置。这个分数就是用户开箱即用所得到的，由为榜上所有其他检查器评分的同一套代码评出。"
---

Python 现在拥有一个真正出色的类型系统，而大多数开发者仍然没有意识到这一点。Python 类型检查器的工作方式很像 TypeScript 编译器。经过类型检查的 Python 之于普通 Python，就如同 TypeScript 之于 JavaScript。类型注解已经存在于语言中十年了，规范已经成熟，工具也已经跟上。

从来悬而未决的问题从不是 Python 的类型系统是否足够好。而是任何一个具体工具究竟对它的实现有多忠实。

本周我们为 Basilisk 得到了一个客观的答案。它被添加进了[官方 python/typing 符合性结果]({{ conformanceOfficial.snapshot.source }})，并取得了完美的 {{ conformanceOfficial.basilisk.pct }}%（{{ conformanceOfficial.basilisk.total }} 项测试中通过 {{ conformanceOfficial.basilisk.passLabel }} 项）。它是榜上唯一达到 {{ conformanceOfficial.basilisk.pct }}% 的类型检查器。

我们为此感到自豪。同时我们也不会过度宣传它，本文接下来的部分会同时解释这句话的两半。

## 为什么符合性测试套件才是那个重要的裁判

工具不能给自己的作业打分。每个类型检查器的作者都会告诉你他们的工具很出色。这不是证据。

[python/typing 符合性测试套件](https://github.com/python/typing/tree/main/conformance)是 Python 生态系统中最接近客观裁判的东西。它由 Python Typing 社区维护，将真正的类型规范编码为一组测试文件，并用同一套评分工具、同样的测试来运行每一个参与的检查器。没有人给自己打分。是这个套件在一次运行中把它们全部一起评分。

这正是结果之所以有意义的原因。当 Basilisk 在那个页面上显示 {{ conformanceOfficial.basilisk.pct }}% 时，那不是我们的声称。那是套件的测量结果，由[给其他所有人评分的同一套工具](https://github.com/python/typing/blob/main/conformance/README.md)产出。

Basilisk 是通过 [python/typing 拉取请求 #2316](https://github.com/python/typing/pull/2316)（"Add Basilisk to conformance results"）加入那次运行的，该请求于 2026 年 7 月 6 日合并。从那一刻起，Basilisk 就在公开场合、以与其他所有工具相同的条件被衡量，你随时都可以自己核对这个数字。

## 当前的排行榜

以下是当前排行榜，转录自 {{ conformanceOfficial.snapshot.dateLabel }} 发布的[官方结果]({{ conformanceOfficial.snapshot.source }})。每个分数都链接到该工具的实时结果文件夹，因为这些数字会随着每个工具的改进而变化，你应当始终能核对当前数字，而不是相信某个快照。

| 排名 | 类型检查器 | 背后团队 | 符合性 |
|---|---|---|---|
{%- for t in conformanceOfficial.ranked %}
| {{ t.rank }} | [{{ t.name }} {{ t.version }}]({{ t.resultsUrl }}) | {{ t.org | default("独立") }} | **{{ t.pct }}%** ({{ t.passLabel }}/{{ t.total }}) |
{%- endfor %}

关于那张表格，有几点值得直白地说清楚，因为 Basilisk 所处的阵容非常有分量。

[Pyright](https://github.com/microsoft/pyright) 由微软开发。[Pyrefly](https://github.com/facebook/pyrefly) 由 Meta 构建。[ty](https://github.com/astral-sh/ty) 由 Astral 构建，也就是 Ruff 和 uv 背后的团队，该团队[已同意加入 OpenAI](https://openai.com/index/openai-to-acquire-astral/)（该交易于 2026 年 3 月宣布，在宣布时仍需监管批准和惯例性交割条件）。[mypy](https://github.com/python/mypy) 是最早的那个，由 Jukka Lehtosalo 创建，并在 Dropbox 大量开发。[zuban](https://github.com/zubanls/zuban) 由 Jedi 的作者 David Halter 编写。[pycroscope](https://github.com/JelleZijlstra/pycroscope) 由 CPython 核心开发者 Jelle Zijlstra 维护。

这些团队拥有真实的人手、真实的预算和深厚的专业知识。其中好几个都很出色，分数也证明了这一点。zuban、Pyrefly 和 Pyright 都在 96% 以上，这真的很难做到。它们没有一个达到 100%。Basilisk 达到了。

我们这么说不是为了炫耀。我们这么说是因为这是套件报告的事实，也是因为一件既奇特又美好的事：一个小型独立工具位居一个包含全球三大软件公司的排行榜之首。这正是一个开放、共享的符合性套件的全部承诺：它不在乎工具背后是谁。它只在乎代码是否正确。

## 100% 意味着什么，又不意味着什么

接下来这部分我们要反驳自己的标题，因为你值得听到诚实的版本。

python/typing 的维护者在结果页面的顶部就放了一个提醒，我们完全同意：

> "虽然规范符合性对生态系统很重要，但我们不建议将其作为选择类型检查器的主要依据。它并不能代表用户通常关心的许多方面。"（[python/typing 符合性结果]({{ conformanceOfficial.snapshot.source }})）

请读两遍。构建这个套件的人正在告诉你，不要把他们自己的记分牌当作唯一重要的东西。这是正确的立场，我们不会为了让 Basilisk 看起来更好而假装不是这样。

那么让我们准确地说清楚，完美的符合性分数是什么，又不是什么。

**它是什么：**它证明当 Basilisk 依据类型规范来评判你的代码时，它的判断是正确的。一个未实现某个规范特性的检查器，无法对使用该特性的代码进行推理。它要么漏掉一个真正的错误，要么发明一个虚假的错误。在这次符合性运行中，Basilisk 捕获了每一个必需的错误，并在整个套件中产生了零误报。这是信任的底线。如果一个检查器对规范的判定不可靠，那么它所做的任何其他事情也都无法依赖。

**它不是什么：**它不是一个声称 Basilisk 自动就是你项目最佳选择的说法。符合性不衡量检查器运行有多快、错误信息有多好、与你的编辑器集成得有多好，或者其生态系统有多成熟。这些方面极其重要，而在其中一些方面，较老的工具有多年的先发优势。

符合性是一个输入。它恰好是决定你能否信任其余一切的那个输入。但它不是全部决定因素，任何告诉你单一数字就能定论的人都在推销某种东西。

## 我们为什么还是追求了这个数字

如果符合性不是全部，我们为什么把 100% 当作一个硬性要求，而不是一个锦上添花的东西？

因为另一种情况是一个有时会自信地出错的检查器，而一个自信地出错的检查器比没有检查器更糟。Python 类型的问题从来不在于语法。问题在于强制执行。一个从不被检查的类型提示只是一句注释。一个被有漏洞的工具检查的类型提示，是一句偶尔会对你撒谎的注释。

Basilisk 的默认规则集就是类型规范，开启所有规范规则、不做任何配置。没有要记住的 `--strict` 标志，因为严格就是底线。当你在自己的代码上运行 Basilisk 时，你得到的判定就是规范所说你应当得到的那个。这就是这个工具的全部意义，而符合性分数正是我们证明自己确实做到了、而非仅仅声称做到的方式。

## 这个分数究竟是如何产生的

我们用那种枯燥、可复现的方式来测量它，因为那是唯一值得发布的测量方式。

Basilisk 的符合性数字来自套件自己未经修改的评分工具，针对默认配置的 Basilisk CLI 运行，开启所有规范规则、不打开任何特殊设置。没有内置的计算器，也没有可能美化结果的自制评分器。给 Basilisk 评分的那套工具，正是给 Pyright、mypy、ty、Pyrefly、zuban 和 pycroscope 评分的同一套 `python/typing` 工具。如果你克隆该套件并自己运行，你会得到同样的榜单。

这是一个刻意的设计选择，它对应着我们对所交付的一切都坚持的一条规则：自我测量的指标只有在可复现、且由中立方测量时才有价值。符合性套件就是那个中立方。我们只是确保我们的工具出现并运行。

## 试用它，并试着让它出错

你可以在我们的[符合性页面](/docs/conformance/)上查看完整的对比，包括分数随时间的变化，也可以在 [python/typing 结果页面]({{ conformanceOfficial.snapshot.source }})上阅读最原始的事实来源。

不过，你能做的最好的事，是把 Basilisk 指向你自己的代码，看看它在哪里和你意见相左。如果它标记了规范认为合法的东西，那就是一个 bug，我们希望在 [GitHub](https://github.com/Nimblesite/Basilisk/issues) 上听到它。Basilisk 之所以达到 {{ conformanceOfficial.basilisk.pct }}%，正是通过把每一个被报告的缺口都当作一个真实的缺陷来修复，一次一个，面对一个不在乎我们感受的裁判。现在我们身处榜首，这一点不会改变。如果有什么不同，那就是它变得更重要了。

Python 的类型系统已经足够好、值得信任有一段时间了。现在，工具也可以了。
