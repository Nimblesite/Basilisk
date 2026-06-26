---
layout: layouts/docs.njk
title: "Basilisk 如何衡量 PEP 符合性"
description: "Basilisk 的 PEP 符合性得分如何用官方 python/typing 符合性套件衡量——套件是什么、评分如何进行、我们运行的字节级一致且 sha256 固定的计算器，以及为何我们在每条规则都启用的情况下评分、从不关闭任何一条。"
keywords: pep 符合性, python 类型符合性套件, basilisk 符合性得分, 类型检查器评分, python/typing 计算器
lang: zh
---
{% from "conformance-chart.njk" import chart %}

# 我们如何衡量 PEP 符合性

Basilisk 由**官方 `python/typing` 符合性套件**评分——也就是类型社区用来为 pyright、mypy、pyrefly、ty 等打分的同一套测试与评分工具。我们在每次改动时，对真实的 `basilisk` 二进制文件原样运行该工具。

目前的结果是 **{{ conformance.scorePct }}%**——{{ conformance.total }} 个测试文件中 **{{ conformance.pass }}** 个通过，捕获 {{ conformance.caught }} 个必需错误，仍有 **{{ conformance.fp }} 处误报**和 **{{ conformance.missed }} 处遗漏的必需错误**待清除。{{ conformance.categoriesTotal }} 个类别中有 {{ conformance.categoriesPass100 }} 个达到 100%。目标是 100%，我们逐步逼近。

<p class="conf-links">
  <a href="https://typing.python.org/en/latest/spec/" target="_blank" rel="noopener">Python 类型规范 ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/README.md" target="_blank" rel="noopener">符合性套件与 README ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/results/results.html" target="_blank" rel="noopener">已发布结果 ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/blob/main/conformance/score.py" target="_blank" rel="noopener">我们的评分器 score.py ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/blob/main/conformance/upstream_main.py" target="_blank" rel="noopener">内置计算器 ↗</a>
</p>

## 符合性套件是什么

[Python 类型规范](https://typing.python.org/en/latest/spec/)定义了类型系统应当如何运作——泛型、协议、`TypedDict`、重载等。为使其名副其实，类型社区在 [`python/typing`](https://github.com/python/typing/tree/main/conformance) 仓库中与规范并行维护着一套**符合性测试套件**。

它的工作方式是：

- 每个规范章节对应一个或多个**测试文件**——普通的 Python 模块，用 `# E` 注释标出每一行符合规范的类型检查器**必须**报告错误的位置（以及用 `# E[tag]` 组标出多个相关错误中报告其一即可的位置）。
- 一个小型**评分工具**对这些文件运行某个类型检查器，并将其输出与注释做差异比对。文件只有在差异为空时才*通过*：每个必需错误都被报告，且没有任何诊断落在套件未标记的行上。
- 维护者用它为每个检查器打分，并发布[结果表](https://github.com/python/typing/blob/main/conformance/results/results.html)——pyright 约 99%、pyrefly 约 86% 等数字便是这样得出的。

我们使用这套套件，固定在提交 [`{{ conformance.pinnedRef }}`](https://github.com/python/typing/tree/{{ conformance.pinnedRef }}/conformance)。同样的工具与文件为所有人打分，因此这个数字在各检查器之间可比，也不是我们能朝有利方向调整的。

## 一个文件如何评分

整个算法就是套件 `main.py` 中的两个函数——`get_expected_errors`（读取 `# E` 注释）与 `diff_expected_errors`（与检查器输出比对）。文件**当且仅当**该差异为空时通过：

- 套件的规则（`upstream_main.py:185`）：`"Fail" if errors_diff.strip() else "Pass"`

我们计入检查器发出的**每一个**诊断——错误*和*警告，**不排除任何代码**。这是最严格的读法，也是 pyright（参考检查器）的评分方式：一个多余的诊断（一处误报）就会让整个文件失败，因此误报数与通过数同样重要。

## 我们如何在不分叉的情况下运行它

套件的 `main.py` 是维护者的批处理工具——它一次性为所有已知检查器打分，且无法调用我们的二进制文件。因此，正如它为每个检查器所做的那样（`PyrightTypeChecker`、`MypyTypeChecker` 等），我们加一个薄薄的**适配器**，复用套件自己的评分而非重新实现。我们的 [`score.py`](https://github.com/Nimblesite/Basilisk/blob/main/conformance/score.py)：

1. **适配器**——运行 `basilisk check --output json`，把结果整理成套件函数期望的 `{line: [errors]}` 字典（这是套件唯一无法替我们做的事）。
2. **计算器**——从一份字节级一致的套件 `main.py` committed 副本中导入 `get_expected_errors` 与 `diff_expected_errors` 并原样调用（`score.py:287` 对应套件自己在 `upstream_main.py:175` 的调用）。它不含任何自己的评分逻辑。
3. **门禁**——将结果与 `coverage-thresholds.json` 比较，任何回归都让 CI 失败。

为保证计算器可信，内置副本经 **sha256 固定**。`score.py` 在每次运行时重新哈希它，若有漂移则拒绝评分（`score.py:99`）；本网站在构建时也会再次重新哈希：

{% if conformance.verified %}
<p><span class="conf-verified">✓ 构建时已校验 —— conformance/upstream_main.py 为 {{ conformance.upstreamBytes }} 字节，sha256 {{ conformance.sha256Short }}…，与固定值一致</span></p>
{% endif %}

适配器与门禁住在另一个可审计的文件里，因此计算器逐字节就是套件自己的那一份。

## 每条规则都运行——不存在"符合性模式"

我们完全按真实用户的运行方式为二进制文件评分：**启用每一条规则，无任何配置，不关闭任何东西。** 在评分之前，`score.py` 会从测试夹具目录中*删除*任何 `basilisk.json`，使任何规则都无法被悄悄静音。为抬高数字而关闭某条规则是被禁止的。

这份诚实是有意为之地让我们损失分数。Basilisk 默认严格，并在其上叠加了类型规范未定义的内部风格规则：主要是要求每个参数、返回值以及 `*args`/`**kwargs` 都带注解，外加一条冗余注解警告、一个缺失 `@override` 的提示，以及一个显式 `Any` 提示。规范将未注解的类型视为**推断**而非错误，因此这些规则会在整套套件上触发并被计为误报。它们构成了如今 **{{ conformance.fp }}** 处误报的绝大部分。

我们本可以靠在评分时关闭这些规则来让明天的数字更好看。我们不会这么做。已发布的数字必须意味着*你开箱即得的真实结果*。通往 100% 的唯一正当途径是把检查器做得更聪明——使其严格的默认规则不再对符合规范的代码触发——并且每条规则仍全部开启。（你完全可以在**你自己的项目里**放宽任何规则；符合性评分器从不这么做。）

## 得分如何变化

本站显示符合性数字已有一段时间；它并非一直诚实，而我们宁愿坦白说明，也不愿悄悄修正。曾有两种不同的衡量捷径一度把它抬高。第一种是一个仓库内脚本，它将若干诊断代码排除在评分之外、且未计入误报。我们用官方计算器替换了它——但随后第二种捷径又溜了进来：评分器以一种"符合性模式"运行二进制文件，在评分前**关闭了六条默认严格的规则**，把报出的数字推到了一个**虚假的 100%**。这恰恰是本页面存在所要防止的那种作弊。

两种捷径都已移除。我们现在以**启用每一条规则**——真实的开箱体验——对二进制文件运行官方计算器，如今诚实的数字是 **{{ conformance.scorePct }}%**（{{ conformance.pass }} / {{ conformance.total }} 个文件，{{ conformance.fp }} 处误报，{{ conformance.missed }} 处遗漏的必需错误）。数字下降不是因为检查器变差，而是因为我们不再用不诚实的方式衡量它。100% 是我们逐步逼近的目标——靠修复检查器，绝不靠关闭某条规则。

下面的图表在构建时直接读取 **`conformance/conformance_status.csv` 的 git 历史**：每个改动该文件的提交对应一个点，绘制该提交实际记录的得分——包括从作弊得来的 100% 跌至诚实数字的那次下降。

{{ chart(conformance, {
  "label": "符合性得分随时间变化",
  "heading": "从关闭规则得来的 100% 到诚实的、每条规则都启用的数字",
  "prevLegend": "在关闭规则或排除代码的情况下衡量（并非真实开箱行为）",
  "officialLegend": "官方 <code>python/typing</code> 计算器，每条规则均启用",
  "dropNote": "早期运行曾报告高达 <strong>" + conformance.chart.peak.score + "%</strong>——但那只是靠关闭规则或排除代码做到的。在每条规则都启用的情况下诚实评分，真实数字是 <strong>" + conformance.chart.current.score + "%</strong>。这次下降是一次更正，而非回归：检查器从未变差，是衡量变诚实了。",
  "caption": "每个点都是对 <code>conformance/conformance_status.csv</code> 的真实提交，每次构建重新计算。悬停某点可查看其日期、提交、得分与误报数。"
}) }}

## 各类别现状

构建时从 `conformance/conformance_status.csv` 实时读取：

<div class="table-wrapper">
<table>
<thead><tr><th>类别</th><th>通过</th><th>得分</th><th></th></tr></thead>
<tbody>
{%- for cat in conformance.categories %}
<tr><td>{{ cat.label }}</td><td>{{ cat.pass }} / {{ cat.total }}</td><td>{{ cat.pct }}%</td><td><span class="conf-cat-bar" style="width: {{ (cat.pct * 1.2) | round }}px; opacity: {{ 0.4 + cat.pct / 170 }}"></span></td></tr>
{%- endfor %}
</tbody>
</table>
</div>

## 自己复现

```bash
# 构建二进制、获取（被 git 忽略的）测试夹具、对其运行官方 python/typing
# 计算器、写出 conformance_status.csv，并强制执行 coverage-thresholds.json 中的棘轮门禁。
make conformance
```

这一切都在两个文件里：[`conformance/score.py`](https://github.com/Nimblesite/Basilisk/blob/main/conformance/score.py)（我们的适配器与门禁）和 [`conformance/upstream_main.py`](https://github.com/Nimblesite/Basilisk/blob/main/conformance/upstream_main.py)（套件的计算器，committed 且经 sha256 固定）。完整的注解规则见 [python/typing 符合性 README](https://github.com/python/typing/blob/main/conformance/README.md)。
