---
layout: layouts/docs.njk
title: 诊断规则
description: 所有 BSK-E（错误）和 BSK-W（警告）诊断代码的完整参考。
keywords: basilisk规则, 类型错误, BSK-E, BSK-W, 诊断代码
lang: zh
---

# 诊断规则

每个 Basilisk 诊断都有一个 `BSK-EXXXX`（错误）或 `BSK-WXXXX`（警告）格式的唯一代码。

所有规则默认启用。没有选择加入。

| 组 | 代码 | 描述 |
|---|---|---|
| [缺失注解](/zh/docs/rules/missing-annotations/) | E0001–E0009 | 未标注的参数、返回类型、变量和属性 |
| [类型安全](/zh/docs/rules/type-safety/) | E0010–E0025 | 类型不匹配、错误的注解、不健全的类型使用 |
| [所有权安全](/zh/docs/rules/ownership-safety/) | E0030–E0035 | Mojo 启发的所有权违规 |
| [不可变性](/zh/docs/rules/immutability/) | E0040–E0043 | 不可变参数和 `Final` 变量的变异 |
| [结构纪律](/zh/docs/rules/structural-discipline/) | E0050–E0054 | 动态属性、缺少 `__init__`、密封类违规 |
| [强制转换安全](/zh/docs/rules/coercion-safety/) | E0060–E0063 | 隐式数字和类型强制转换 |
| [可选安全](/zh/docs/rules/optional-safety/) | E0070–E0073 | 对 `Optional` 值的不安全访问 |
| [未使用代码](/zh/docs/rules/unused-code/) | W0080–W0089 | 未使用的导入、变量、函数和不可达分支 |
| [代码质量](/zh/docs/rules/code-quality/) | W0090–W0099 | 抑制注释、弃用 API、可变默认值 |
