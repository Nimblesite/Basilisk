---
layout: layouts/blog.njk
title: "已撤回：Basilisk 此前的类型符合性结果"
description: "撤回 Basilisk 此前的 Python typing 符合性声明，说明该结果为何不可信，以及受影响的规则为何是被删除而不是被修补。"
date: 2026-07-11
dateModified: 2026-08-06
author: Christian Findlay
image: /assets/images/og-image.png
imageAlt: "Basilisk Python 类型检查器与语言服务器；结果正在接受完整性审查"
imageWidth: 1200
imageHeight: 630
tags:
  - Python typing
category: announcements
lang: zh
excerpt: "Basilisk 已撤回此前的符合性声明，并请求从官方结果中移除。本文仅作为已撤回公告的历史记录保留。"
keywords: python类型检查器, python类型符合性, python/typing符合性结果, basilisk, mypy, pyright, ty, pyrefly, zuban, pep符合性, 严格类型
faq:
  - q: "哪个 Python 类型检查器的符合性分数最高？"
    a: "Basilisk 目前不在官方 python/typing 结果中。此前的结果已撤回；在完成规则审计、删除依据源文本判断的规则之前，实际百分比暂时未知。其他工具请查看官方实时结果表。"
  - q: "什么是 python/typing 符合性测试套件？"
    a: "这是由 Python Typing 社区维护的官方测试套件。它使用自己的评分工具记录检查器在确切测试用例上的行为。这是有价值的证据，但原始套件结果本身不能证明完整规范已被忠实实现；还必须通过保持语义的变异和独立的套件外用例验证。"
  - q: "100% 的符合性分数是否等于是最好的类型检查器？"
    a: "不。套件分数只描述被覆盖的测试用例；正如 Basilisk 此次撤回所证明的，它本身不能证明规范实现正确。它也无法反映编辑器集成、错误信息质量、生态系统支持或经过独立验证的性能。"
  - q: "Basilisk 的符合性分数是如何测量的？"
    a: "Basilisk 目前没有可发布的符合性分数。只有在审计完成、依据源文本而非已解析符号判断的规则被删除之后，未来结果才可能发布，并且必须同时通过未经修改的 python/typing 评分工具、保持语义的变异测试，以及依据规范独立设计的套件外用例。"
---

> **撤回说明——2026 年 8 月 6 日：**我们撤回本文中的所有符合性声明。Basilisk 源码中存在针对确切符合性测试用例实现的逻辑，因此此前的满分结果不能证明规范符合性。我们请求从官方结果表中移除 Basilisk，现已完成移除。在删除有问题的实现、根据规范重新构建并通过保持语义的变异测试之前，当前百分比暂时未知。下方原文仅作为公开历史记录保留；其中的得分、排名、通过数量和结论均不可依赖。请阅读[完整更正](/zh/docs/conformance/)。

Python 现在拥有一个真正出色的类型系统，而大多数开发者仍然没有意识到这一点。Python 类型检查器的工作方式很像 TypeScript 编译器。经过类型检查的 Python 之于普通 Python，就如同 TypeScript 之于 JavaScript。类型注解已经存在于语言中十年了，规范已经成熟，工具也已经跟上。

从来悬而未决的问题从不是 Python 的类型系统是否足够好。而是任何一个具体工具究竟对它的实现有多忠实。

发布本文时，我们以为 Basilisk 得到了一个客观答案。它被添加进这份[官方 python/typing 符合性结果的固定快照]({{ conformanceOfficial.historical.snapshot.snapshotUrl }})，当时的运行报告了 {{ conformanceOfficial.historical.basilisk.pct }}%（{{ conformanceOfficial.historical.basilisk.total }} 项测试中通过 {{ conformanceOfficial.historical.basilisk.passLabel }} 项）。该结果现已撤回。

我们曾为此感到自豪。完整性审计证明这个结论是错误的。

## 为什么符合性测试套件才是那个重要的裁判

工具不能给自己的作业打分。每个类型检查器的作者都会告诉你他们的工具很出色。这不是证据。

[python/typing 符合性测试套件](https://github.com/python/typing/tree/main/conformance)是 Python 生态系统中最接近客观裁判的东西。它由 Python Typing 社区维护，将真正的类型规范编码为一组测试文件，并用同一套评分工具、同样的测试来运行每一个参与的检查器。没有人给自己打分。是这个套件在一次运行中把它们全部一起评分。

我们曾认为这使结果具有意义。套件确实使用共享评分工具产出了该数字，但我们的代码针对确切测试文本进行了适配，因此该测量无法支撑我们得出的结论。

Basilisk 是通过 [python/typing 拉取请求 #2316](https://github.com/python/typing/pull/2316)（"Add Basilisk to conformance results"）加入那次运行的，该请求于 2026 年 7 月 6 日合并。撤回结果后，我们请求将其移除；Basilisk 已不再出现在实时结果表中。

## 当时发布的历史排行榜快照

以下是原公告在 {{ conformanceOfficial.historical.snapshot.dateLabel }} 使用的排行榜快照。它不是当前结果，Basilisk 这一行也已撤回。当前仍列出的工具请查看[官方实时结果]({{ conformanceOfficial.historical.snapshot.source }})。

| 排名 | 类型检查器 | 背后团队 | 符合性 |
|---|---|---|---|
{%- for t in conformanceOfficial.historical.ranked %}
| {{ t.rank }} | [{{ t.name }} {{ t.version }}]({{ t.resultsUrl }}) | {{ t.org | default("独立") }} | **{{ t.pct }}%** ({{ t.passLabel }}/{{ t.total }}) |
{%- endfor %}

关于那张表格，有几点值得直白地说清楚，因为 Basilisk 所处的阵容非常有分量。

[Pyright](https://github.com/microsoft/pyright) 由微软开发。[Pyrefly](https://github.com/facebook/pyrefly) 由 Meta 构建。[ty](https://github.com/astral-sh/ty) 由 Astral 构建，也就是 Ruff 和 uv 背后的团队，该团队[已同意加入 OpenAI](https://openai.com/index/openai-to-acquire-astral/)（该交易于 2026 年 3 月宣布，在宣布时仍需监管批准和惯例性交割条件）。[mypy](https://github.com/python/mypy) 是最早的那个，由 Jukka Lehtosalo 创建，并在 Dropbox 大量开发。[zuban](https://github.com/zubanls/zuban) 由 Jedi 的作者 David Halter 编写。[pycroscope](https://github.com/JelleZijlstra/pycroscope) 由 CPython 核心开发者 Jelle Zijlstra 维护。

发布时，我们用该快照将 Basilisk 排在其他工具之前。由于 Basilisk 的结果缺乏稳健性，这项比较现已撤回。

原文将该快照描述为一个小型独立工具位居大型团队之前的证明。这个说法属于已撤回的声明。

## 100% 意味着什么，又不意味着什么

接下来这部分我们要反驳自己的标题，因为你值得听到诚实的版本。

python/typing 的维护者在结果页面的顶部就放了一个提醒，我们完全同意：

> "虽然规范符合性对生态系统很重要，但我们不建议将其作为选择类型检查器的主要依据。它并不能代表用户通常关心的许多方面。"（[python/typing 符合性结果]({{ conformanceOfficial.historical.snapshot.source }})）

请读两遍。构建这个套件的人正在告诉你，不要把他们自己的记分牌当作唯一重要的东西。这是正确的立场，我们不会为了让 Basilisk 看起来更好而假装不是这样。

原文试图解释我们当时认为满分意味着什么。完整性审计推翻了核心结论。

**我们曾声称它是什么：**证明 Basilisk 能根据类型规范正确判断代码。这个推论是错误的。如果检查器的一部分匹配了测试文本，通过确切套件并不能证明通用实现。

**它不是什么：**它不是一个声称 Basilisk 自动就是你项目最佳选择的说法。符合性不衡量检查器运行有多快、错误信息有多好、与你的编辑器集成得有多好，或者其生态系统有多成熟。这些方面极其重要，而在其中一些方面，较老的工具有多年的先发优势。

符合性是一个输入。它恰好是决定你能否信任其余一切的那个输入。但它不是全部决定因素，任何告诉你单一数字就能定论的人都在推销某种东西。

## 我们为什么还是追求了这个数字

如果符合性不是全部，我们为什么把 100% 当作一个硬性要求，而不是一个锦上添花的东西？

因为另一种情况是一个有时会自信地出错的检查器，而一个自信地出错的检查器比没有检查器更糟。Python 类型的问题从来不在于语法。问题在于强制执行。一个从不被检查的类型提示只是一句注释。一个被有漏洞的工具检查的类型提示，是一句偶尔会对你撒谎的注释。

Basilisk 默认启用类型规范规则，无需记住 `--strict` 标志。我们曾声称旧分数证明这些规则正确实现了规范。事实并非如此；相关实现正在重新构建和验证。

## 已撤回分数如何产生

我们用那种枯燥、可复现的方式来测量它，因为那是唯一值得发布的测量方式。

已撤回的数字来自套件自己未经修改的评分工具，针对默认配置的 Basilisk CLI 运行并开启所有规范规则。该过程可以复现数字，却无法揭示部分实现针对确切测试进行了适配。因此，未来发布必须同时通过官方评分工具和基于变异的稳健性检查。

这是一个刻意的设计选择，它对应着我们对所交付的一切都坚持的一条规则：自我测量的指标只有在可复现、且由中立方测量时才有价值。符合性套件就是那个中立方。我们只是确保我们的工具出现并运行。

## 试用它，并试着让它出错

你可以在[符合性页面](/zh/docs/conformance/)阅读当前更正和修复计划，并在 [python/typing 结果页面]({{ conformanceOfficial.historical.snapshot.source }})上看到 Basilisk 已不再列出。

请把 Basilisk 指向你自己的代码，并在 [GitHub](https://github.com/Nimblesite/Basilisk/issues) 上报告分歧。旧得分不能替代这种真实检验。只有在审计完成、留下的代码通过更广泛的回归用例和保持语义的变异后，我们才会发布替代结果。

Python 的类型系统已经足够好、值得信任有一段时间了。现在，工具也可以了。
