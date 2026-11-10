---
layout: layouts/docs.njk
title: "Basilisk 在官方 python/typing 一致性套件中取得满分 100%"
description: "Basilisk 是唯一在官方 python/typing 一致性测试结果中取得满分 100% 的 Python 类型检查器——就发布在 Python typing 仓库的结果页面上，领先于 Pyright、mypy、Pyrefly 和 ty。这里是证据以及衡量方式。"
keywords: pep 符合性, python/typing 一致性结果, 100% 符合的类型检查器, basilisk 符合性得分, python/typing 结果
lang: zh
---
{% from "conformance-chart.njk" import chart %}

# Basilisk 取得满分 100%

Basilisk 是**唯一在[官方 `python/typing` 一致性测试结果](https://github.com/python/typing/blob/main/conformance/results/results.html)中取得满分 {{ conformanceOfficial.byId.basilisk.pct }}%** 的 Python 类型检查器——而且它**就发布在 Python typing 仓库自己的结果页面上**，与其他所有检查器在同一次运行中评分。

<p class="conf-links">
  <a href="https://github.com/python/typing/blob/main/conformance/results/results.html" target="_blank" rel="noopener"><strong>官方 python/typing 结果 ↗</strong></a>
  <a href="{{ conformanceOfficial.snapshot.addedPrUrl }}" target="_blank" rel="noopener">收录 Basilisk 的 PR ↗</a>
  <a href="https://typing.python.org/en/latest/spec/" target="_blank" rel="noopener">Python 类型规范 ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/README.md" target="_blank" rel="noopener">符合性 README ↗</a>
</p>

## 官方排行榜

下面每个得分都来自[官方 `python/typing` 一致性套件](https://github.com/python/typing/blob/main/conformance/results/results.html)的**同一次运行**——也就是类型社区用来为每个检查器打分的同一套件与评分器。Basilisk 位居榜首，也是**榜单上唯一取得满分**的工具。

<div class="table-wrapper">
<table>
<thead><tr><th>工具</th><th>背后机构</th><th>官方符合性</th></tr></thead>
<tbody>
{%- for t in conformanceOfficial.ranked %}
<tr{% if t.id == "basilisk" %} class="conf-row-basilisk"{% endif %}>
  <td>{% if t.id == "basilisk" %}<strong>Basilisk</strong>{% else %}{{ t.name }}{% endif %}</td>
  <td>{{ t.org or "独立" }}</td>
  <td><a href="{{ t.resultsUrl }}" target="_blank" rel="noopener">{% if t.id == "basilisk" %}<strong>{{ t.pct }}%（{{ t.passLabel }}/{{ t.total }}）</strong>{% else %}{{ t.pct }}%{% endif %}</a></td>
</tr>
{%- endfor %}
</tbody>
</table>
</div>

<p class="conf-note"><a href="{{ conformanceOfficial.snapshot.snapshotUrl }}" target="_blank" rel="noopener">results.html</a> 在 <a href="{{ conformanceOfficial.snapshot.commitUrl }}" target="_blank" rel="noopener">python/typing@<code>{{ conformanceOfficial.snapshot.sha }}</code></a>（{{ conformanceOfficial.snapshot.dateLabel }}）的快照。这些数字会随其他工具的改进而变化，因此每个单元格都链接到该工具的**实时**结果目录——请自行核对当前数字。</p>

## 如何衡量

我们不用自己的尺子给自己打分。上面的数字由**官方 `python/typing` harness** 产生，对通过 **wheel 安装的 `basilisk` 命令**原样运行——也就是你从 PyPI 得到的同一个 CLI，在其**默认配置**下，**每条 PEP 符合性规则都开启、别无其他配置**。文件只有在 harness 的差异为空时才通过：每个必需错误都被报告，且**没有**诊断落在套件未标记的行上。我们计入检查器发出的每一个诊断——错误*和*警告——因此一处误报就会让整个文件失败。

目前的结果是 **{{ conformance.scorePct }}%**——{{ conformance.total }} 个测试文件中 **{{ conformance.pass }}** 个通过，捕获 {{ conformance.caught }} 个必需错误，**{{ conformance.fp }} 处误报**、**{{ conformance.missed }} 处遗漏**。我们与 `python/typing@main` 步调一致（针对 [`{{ conformance.pinnedRefShort }}`](https://github.com/python/typing/tree/{{ conformance.pinnedRef }}/conformance){% if conformance.commitDate %}，{{ conformance.commitDate }}{% endif %} 评分）；棘轮门禁防止得分回退，任何我们未通过的上游测试都会阻止合并与发布。

Basilisk 的**可选内建规则**（要求注解、冗余注解、缺失 `@override`、显式 `Any`）在评分中从不运行——全新安装一条都不会启用，开启它们只会*拉低*得分，因为规范将未注解的值视为*推断*而非错误。"比规范更严格"与"符合规范"是不同的目标；这个得分只衡量后者。

### 自己复现

Basilisk 是官方套件中的**已注册检查器**——`BasiliskTypeChecker` 就在 `python/typing` 的 [`conformance/src/type_checker.py`](https://github.com/python/typing/blob/main/conformance/src/type_checker.py) 中——所以你直接运行真实 harness，无需任何修补：

```bash
# 全新克隆 python/typing，用它自己的 harness 针对 basilisk 二进制运行，
# 并从真实结果重新生成 conformance/conformance_status.csv。
python3 conformance/run_conformance.py --bin target/release/basilisk
```

或针对 PATH 上任意 `basilisk` 手动驱动上游 harness：

```bash
git clone --depth 1 https://github.com/python/typing
BASILISK_BIN=$(which basilisk) python typing/conformance/src/main.py --only-run basilisk
```

运行器位于 [`conformance/run_conformance.py`](https://github.com/Nimblesite/Basilisk/blob/main/conformance/run_conformance.py)；它全新克隆套件、运行未经修改的上游 harness，自身从不进行任何评分。

## 得分如何变诚实

我们宁愿坦白说明也不愿掩盖。早期的一个仓库内脚本曾通过**把若干诊断代码排除在差异比对之外、且完全不计入误报**来抬高数字。我们弃用了它，改用官方 `python/typing` 评分语义，对真实的默认 CLI 运行。下面的图表在构建时直接读取 **`conformance/conformance_status.csv` 的 git 历史**——每个改动该文件的提交对应一个点，包括那次更正。

{{ chart(conformance, {
  "label": "符合性得分随时间变化",
  "heading": "从仓库内脚本到官方 harness",
  "prevLegend": "早期仓库内脚本——排除代码、忽略误报（并非官方衡量方式）",
  "officialLegend": "官方 <code>python/typing</code> harness，对真实默认 CLI 运行",
  "dropNote": "早期的点来自一个排除诊断代码、且不计入误报的仓库内脚本；之后的点使用官方 <code>python/typing</code> 评分语义、对真实默认 CLI 运行。今天的官方数字是 <strong>" + conformance.chart.current.score + "%</strong>——是衡量变诚实了，而非检查器变差了。",
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
</content>
