---
layout: layouts/docs.njk
title: 不可变性 — E0040–E0043
description: 强制执行参数和 Final 变量不可变性的规则。
keywords: basilisk, 不可变性, final, inout, BSK-E0040, BSK-E0043
lang: zh
---

# 不可变性 — E0040–E0043

← [所有权安全](/zh/docs/rules/ownership-safety/) | 下一个：[结构纪律](/zh/docs/rules/structural-discipline/) →

---

### BSK-E0040 — 不可变参数的变异

没有 `InOut` 注解的参数被变异。

```python
def process(items: list[int]) -> None:
    items.append(99)  # 错误——items 默认是不可变的
```

通过声明意图来修复：

```python
from typing import Annotated
from basilisk import InOut

def process(items: Annotated[list[int], InOut]) -> None:
    items.append(99)  # OK——InOut 声明了变异意图
```

---

### BSK-E0041 — 不可变参数的重新赋值

参数在函数主体内被重新赋值为新值。

---

### BSK-E0042 — 可变数据类（建议使用 `frozen=True`）

定义了没有 `frozen=True` 的 `@dataclass`。冻结的数据类更安全，并且与 Basilisk 的不可变性模型配合良好。

---

### BSK-E0043 — `Final` 变量的变异

用 `Final` 注解的变量被重新赋值。
