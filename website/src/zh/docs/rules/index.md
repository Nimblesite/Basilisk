---
layout: layouts/docs.njk
title: "Basilisk 诊断规则——Python 类型规范与可选规则标签"
description: "按检查器的权威标签浏览所有 Basilisk 诊断：默认启用的 Python 类型规范规则，以及按标签显式启用的 Basilisk 可选规则。"
keywords: basilisk规则, python类型规则, 严格性规则, 类型错误, BSK-E, BSK-W, 诊断代码
date: 2026-02-28
dateModified: 2026-07-11
author: Basilisk 项目
lang: zh
---
{% from "components/rules.njk" import groupGrid with context %}

# 诊断规则

Basilisk 使用检查器自身的平面标签组织规则，而不是人为的代码范围。每条规则都有且仅有一个来源标签：

- `pep` 表示**默认启用的 {{ ruleTagGroups.counts.pep }} 条核心规则**，其中包括[官方 Python 类型符合性套件](https://github.com/python/typing/blob/main/conformance/results/results.html)测量的规则。
- `basilisk` 表示**默认关闭的 {{ ruleTagGroups.counts.basilisk }} 条 Basilisk 特有规则**，只有项目选择相应描述标签后才会启用。

每个诊断都链接到永久的 `/errors/.../` 说明页，与 CLI 报错时打印的网址完全相同。

## Basilisk 可选规则

当您需要超出类型规范的检查时，按下列标签启用。一条规则可以属于多个描述标签。

{{ groupGrid(ruleTagGroups.basilisk, "zh") }}

## Python 类型规范规则

这些规则构成默认检查面。分类标签直接来自 `python/typing` 符合性词汇；横跨多个分类的检查收录在**跨领域核心规则**中。

{{ groupGrid(ruleTagGroups.pep, "zh") }}

## 完整诊断参考

浏览[按标签分组的错误参考](/errors/)，或直接访问 `/errors/CODE/` 下的任意规范页面。规则选择与严重级覆盖请参阅 [`pyproject.toml` 配置参考](/zh/docs/configuration/)。
