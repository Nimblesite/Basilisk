---
layout: layouts/docs.njk
title: "Basilisk 如何衡量 PEP 符合性"
description: "Basilisk 的 PEP 符合性得分如何用官方 python/typing 符合性套件衡量——套件是什么、评分如何进行、如何用 wheel 安装的 CLI 提交上游，以及为何我们在每条规则都启用的情况下评分、从不关闭任何一条。"
keywords: pep 符合性, python 类型符合性套件, basilisk 符合性得分, 类型检查器评分, python/typing harness
lang: zh
---
{% from "conformance-chart.njk" import chart %}

# 我们如何衡量 PEP 符合性

Basilisk 由**官方 `python/typing` 符合性套件**评分——也就是类型社区用来为 pyright、mypy、pyrefly、ty 等打分的同一套测试与评分工具。发布证明通过 wheel 安装的 `basilisk` 命令原样运行该工具，也就是用户从 PyPI 得到的同一个入口。

目前的结果是 **{{ conformance.scorePct }}%**——{{ conformance.total }} 个测试文件中 **{{ conformance.pass }}** 个通过，捕获 {{ conformance.caught }} 个必需错误，**{{ conformance.fp }} 处误报**、**{{ conformance.missed }} 处遗漏的必需错误**。{{ conformance.categoriesTotal }} 个类别中有 {{ conformance.categoriesPass100 }} 个达到 100%，并由棘轮门禁防止其回退。

<p class="conf-links">
  <a href="https://typing.python.org/en/latest/spec/" target="_blank" rel="noopener">Python 类型规范 ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/README.md" target="_blank" rel="noopener">符合性套件与 README ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/results/results.html" target="_blank" rel="noopener">已发布结果 ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/blob/main/scripts/prepare_typing_conformance_pr.py" target="_blank" rel="noopener">提交脚本 ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/blob/main/docs/typing-conformance-pr.md" target="_blank" rel="noopener">PR 流程 ↗</a>
</p>

## 符合性套件是什么

[Python 类型规范](https://typing.python.org/en/latest/spec/)定义了类型系统应当如何运作——泛型、协议、`TypedDict`、重载等。为使其名副其实，类型社区在 [`python/typing`](https://github.com/python/typing/tree/main/conformance) 仓库中与规范并行维护着一套**符合性测试套件**：普通的 Python 模块，用 `# E` 注释标出每一行符合规范的检查器**必须**报告错误的位置。一个评分工具将检查器的输出与这些注释做差异比对，维护者据此为每个主流检查器发布[结果表](https://github.com/python/typing/blob/main/conformance/results/results.html)。

我们针对上次从 `main` 拉取的套件的确切提交评分——[`{{ conformance.pinnedRefShort }}`](https://github.com/python/typing/tree/{{ conformance.pinnedRef }}/conformance){% if conformance.commitDate %}，{{ conformance.commitDate }}{% endif %}——以完整哈希记录，链接始终固定指向我们所评分的确切文件。

这个固定提交不会过期：我们与 `python/typing@main` **步调一致**。每次 `make test`、每次检查器的 CI 运行以及发布流水线中的专门任务，都会重新解析 `main` 的*当前*最新提交，套件有更新就重新下载，并以 **100% 通过、0 误报** 重新评分——任何我们未通过的上游测试都会阻止合并与发布，直到检查器符合为止。该提交由符合性报告自动写入每个页面，绝非手工键入。

## 一个文件如何评分

官方 harness 会读取套件中的 `# E` 注释，运行检查器适配器，并比对期望诊断与实际诊断。文件只有在这个差异为空时才通过：每个必需错误都被报告，且没有诊断落在未标记的行上。我们计入检查器发出的**每一个**诊断——错误*和*警告，不排除任何代码——因此一处误报就会让整个文件失败。

## 我们如何在不分叉的情况下运行它

上游套件只会为其 `type_checker.py` 中注册的检查器评分。发布与提交证明使用 [`scripts/prepare_typing_conformance_pr.py`](https://github.com/Nimblesite/Basilisk/blob/main/scripts/prepare_typing_conformance_pr.py) 修补一个全新的 `python/typing` checkout，内容正是上游 PR 需要的形态：

1. **适配器**——注入 `BasiliskTypeChecker`，运行 `basilisk check . --output json --color never`，并把所有非空诊断交给上游解析器。
2. **Wheel 安装**——加入 `basilisk-python` 依赖，刷新 `uv.lock`，并确认 `basilisk --version` 来自 `python/typing` 虚拟环境。
3. **Harness**——运行 `uv run --python 3.12 python src/main.py --only-run basilisk`，再运行 `--report-only`，生成上游的 `results/basilisk/*.toml` 与 `results.html`。

这才是提交路径。旧的 Basilisk 本地评分器只是开发快捷检查；它不是发布或上游符合性 PR 的事实来源。

## 得分衡量什么——又从不运行什么

我们完全按真实用户从 wheel 运行 CLI 的方式评分：**默认配置**，即**核心 PEP 符合性规则集**——别无其他。发布与提交门禁在干净的 `python/typing` checkout 中通过 wheel 安装的 `basilisk` 命令运行，因此仓库本地配置既无法静音某条符合性规则，也无法悄悄开启额外规则。为抬高数字而关闭某条符合性规则是被禁止的——删除或注销它同样被禁止。

Basilisk 的**可选规则**（要求注解、冗余注解、缺失 `@override`、显式 `Any`）在评分中从不运行；全新安装一条都不会启用。开启它们只会*拉低*得分而非抬高：规范将未注解的值视为*推断*而非错误，因此"要求注解"会在符合规范的代码上触发，计为一处**误报**。"比规范更严格"与"符合规范"是不同的目标——这个得分只衡量后者。

## 得分如何变化

这个数字并非一直被诚实地衡量，我们宁愿坦白说明也不愿掩盖。早期的一个仓库内脚本曾通过**把若干诊断代码排除在差异比对之外、且完全不计入误报**来抬高数字；我们弃用了它，改用官方 `python/typing` 评分语义，对真实的默认 CLI 运行。下面的图表在构建时直接读取 **`conformance/conformance_status.csv` 的 git 历史**——每个改动该文件的提交对应一个点，包括那次更正。

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

## 自己复现

```bash
# 在 Basilisk checkout 中修补 python/typing checkout，从 basilisk-python
# wheel 安装 basilisk，运行真实上游 harness，并写出证明日志。
python3 scripts/prepare_typing_conformance_pr.py \
  --typing-repo ../typing \
  --verbose \
  --write-proof
```

提交流程位于 [`scripts/prepare_typing_conformance_pr.py`](https://github.com/Nimblesite/Basilisk/blob/main/scripts/prepare_typing_conformance_pr.py)，上游 PR 应包含的文件记录在 [`docs/typing-conformance-pr.md`](https://github.com/Nimblesite/Basilisk/blob/main/docs/typing-conformance-pr.md)。完整的注解规则见 [python/typing 符合性 README](https://github.com/python/typing/blob/main/conformance/README.md)。
