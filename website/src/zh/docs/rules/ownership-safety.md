---
layout: layouts/docs.njk
title: 所有权安全 — E0030–E0035
description: Mojo 启发的所有权分析规则。
keywords: basilisk, 所有权, borrowed, owned, inout, BSK-E0030, BSK-E0031
lang: zh
eleventyNavigation:
  key: Ownership Safety
  parent: Rules
  order: 3
---

# 所有权安全 — E0030–E0035

Mojo 启发的所有权分析。请参阅 [Mojo 风格安全](/zh/docs/mojo-safety/) 以获取完整文档。

← [类型安全](/zh/docs/rules/type-safety/) | 下一个：[不可变性](/zh/docs/rules/immutability/) →

---

### BSK-E0030 — `Borrowed` 参数的变异

`Borrowed` 参数（只读引用）被变异。

```python
from typing import Annotated
from basilisk import Borrowed

def summarise(items: Annotated[list[int], Borrowed]) -> int:
    items.append(99)  # 错误——不能变异 Borrowed
    return sum(items)
```

---

### BSK-E0031 — 所有权转移后使用

在所有权已转移到另一个绑定或函数之后使用了值。

```python
from typing import Annotated
from basilisk import Owned

def consume(data: Annotated[list[int], Owned]) -> int:
    return sum(data)

items = [1, 2, 3]
total = consume(items)
items.append(4)  # 错误——items 已移入 consume()
```

---

### BSK-E0032 — 大型结构的隐式复制

大型结构（超过配置的大小阈值）被隐式复制。需要显式的 `.copy()` 或 `Owned` 转移。

---

### BSK-E0033 — 缺少所有权注解（警告提升为错误）

接收可变类型的函数参数没有所有权注解。Basilisk 无法确定预期的契约。

---

### BSK-E0034 — Owned 值未被消费或返回

创建了一个 `Owned` 值，但既未消费也未返回，泄漏了资源。

---

### BSK-E0035 — 多个可变引用

同一值的两个 `InOut` 引用同时存在。
