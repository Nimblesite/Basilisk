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

目前的结果是 **{{ conformance.scorePct }}%**——{{ conformance.total }} 个测试文件中 **{{ conformance.pass }}** 个通过，捕获 {{ conformance.caught }} 个必需错误，**{{ conformance.fp }} 处误报**、**{{ conformance.missed }} 处遗漏的必需错误**。{{ conformance.categoriesTotal }} 个类别中有 {{ conformance.categoriesPass100 }} 个达到 100%，并由棘轮门禁防止其回退。

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
- 维护者用它为每个检查器打分，并发布[结果表](https://github.com/python/typing/blob/main/conformance/results/results.html)——这是 pyright、mypy、pyrefly、ty 等工具当前得分的实时权威来源。

我们针对上次从 `main` 拉取的套件的确切提交评分——[`{{ conformance.pinnedRefShort }}`](https://github.com/python/typing/tree/{{ conformance.pinnedRef }}/conformance){% if conformance.commitDate %}，{{ conformance.commitDate }}{% endif %}——以完整哈希记录，因此即使 `main` 继续推进，此链接也始终固定指向我们所评分的确切文件。同样的工具与文件为所有人打分，因此这个数字在各检查器之间可比，也不是我们能朝有利方向调整的。

这个记录在案的提交是一张凭据，而不是挡箭牌——我们与 `python/typing@main` **步调一致**。每次 `make test`、每次检查器的 CI 运行，以及每条发布流水线中的专门任务，都会重新解析 `main` 的*当前*最新提交，套件一旦更新就重新下载，并以固定在 **100% 通过、0 误报** 的门槛为真实二进制重新评分。一旦维护者新增或修改测试导致 Basilisk 不再通过，我们的 CI 与发布流程会立即失败：在检查器符合新套件之前，任何代码都无法合并、任何版本都无法发布。上面引用的提交——由评分器自己的报告自动写入每个页面，绝非手工键入——只是记录哪个最新版本得出了这个分数，方便你审计数字背后的确切文件。

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

## 得分衡量什么——又从不运行什么

我们完全按真实用户的运行方式、在其**默认配置**下为二进制文件评分。Basilisk **纯粹依据配置**来决定运行哪些规则，而默认配置恰好与**核心 PEP 符合性规则集完全一致**——别无其他。在评分之前，`score.py` 会从测试夹具目录中*删除*任何 `basilisk.json`，因此配置文件既无法静音某条符合性规则，也无法悄悄开启额外规则。为抬高数字而关闭某条符合性规则是被禁止的——*删除*其源文件或将其从检查器中注销同样被禁止，那只是换个途径作弊。

Basilisk 还附带**可选的 Basilisk 规则**——规范未定义的额外检查，例如要求每个参数、返回值以及 `*args`/`**kwargs` 都带注解，外加一条冗余注解警告、一个缺失 `@override` 的提示，以及一个显式 `Any` 提示。它们**仅在你于配置中启用时**才生效；全新安装一条都不会运行。它们**不是**符合性规则，符合性评分从不执行它们，也从未为得分加上——或减去——哪怕一分。

恰恰相反，开启它们会**破坏** PEP 符合性。规范将未注解的值视为**推断**而非错误——因此像"要求注解"这样的规则会在完全符合规范的代码上触发，并被计为对套件的一处**误报**。这正是这些规则默认关闭、以及符合性以默认配置衡量的原因：纯粹的二进制文件、核心 PEP 规则集，别无其他。当你想要*比规范更严格*的检查时，可在自己的项目中启用这些额外规则——但请明白，"比规范更严格"与"100% 符合规范"是不同的目标，而这个得分只衡量后者。

## 得分如何变化

本站显示符合性数字已有一段时间，而它并非一直被诚实地衡量——我们宁愿坦白说明，也不愿悄悄掩盖。早期的一个仓库内脚本曾通过**把若干诊断代码排除在差异比对之外、且完全不计入误报**来抬高数字，于是本应失败的文件被算作通过。我们弃用了它，改用官方 `python/typing` 计算器，对真实的默认二进制文件原样运行。

你今天看到的就是那个官方数字：**{{ conformance.scorePct }}%**（{{ conformance.pass }} / {{ conformance.total }} 个文件，{{ conformance.fp }} 处误报，{{ conformance.missed }} 处遗漏的必需错误）——每条符合性规则均已启用，而可选的 Basilisk 规则则保持全新安装时的状态：关闭。这个数字就是规范对开箱即用二进制文件给出的结果，没有任何向我们有利的调校。

下面的图表在构建时直接读取 **`conformance/conformance_status.csv` 的 git 历史**：每个改动该文件的提交对应一个点，绘制该提交实际记录的得分——包括我们从仓库内脚本切换到官方计算器时的那次更正。

{{ chart(conformance, {
  "label": "符合性得分随时间变化",
  "heading": "从仓库内脚本到官方计算器",
  "prevLegend": "早期仓库内脚本——排除代码、忽略误报（并非官方衡量方式）",
  "officialLegend": "官方 <code>python/typing</code> 计算器，对真实默认二进制文件运行",
  "dropNote": "早期的点来自一个排除诊断代码、且不计入误报的仓库内脚本；之后的点使用官方 <code>python/typing</code> 计算器、对真实默认二进制文件运行。今天的官方数字是 <strong>" + conformance.chart.current.score + "%</strong>——是衡量变诚实了，而非检查器变差了。",
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
