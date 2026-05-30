---
layout: layouts/docs.njk
title: 诊断规则
description: 所有 Basilisk 诊断代码（BSK-E 错误和 BSK-W 警告）的完整参考。缺失注解、类型安全等。
keywords: basilisk规则, 类型错误, BSK-E, BSK-W, 诊断代码
lang: zh
---

# 诊断规则

每个 Basilisk 诊断都有一个 `BSK-EXXXX`（错误）或 `BSK-WXXXX`（警告）格式的唯一代码。

规则默认全部启用。您可以通过编辑器或 `pyproject.toml`，按文件或路径将单个规则调低——严格是默认值，而不是牢笼。

Basilisk 实现了 150 多个诊断代码，覆盖完整的 Python 类型表面（泛型、协议、dataclass、TypedDict、重载、字面量等），由[官方 Python 类型符合性套件](https://github.com/python/typing/blob/main/conformance/results/results.html)驱动。下面记录了两个基础组；完整集合由检查器强制执行。

| 组 | 代码 | 描述 |
|---|---|---|
| [缺失注解](/zh/docs/rules/missing-annotations/) | E0001–E0009 | 未标注的参数、返回类型、变量和属性 |
| [类型安全](/zh/docs/rules/type-safety/) | E0010–E0029 | 类型不匹配、错误的注解、不健全的类型使用 |

> **路线图：** Mojo 启发的所有权与不可变性分析计划在未来版本中推出。它尚未包含在当前发布的规则集中。
