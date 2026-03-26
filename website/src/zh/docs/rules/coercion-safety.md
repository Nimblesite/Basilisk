---
layout: layouts/docs.njk
title: 强制转换安全 — E0060–E0063
description: 防止隐式数字和类型强制转换的规则。
keywords: basilisk, 强制转换, int, float, bool, bytes, BSK-E0060, BSK-E0063
lang: zh
eleventyNavigation:
  key: Coercion Safety
  parent: Rules
  order: 6
---

# 强制转换安全 — E0060–E0063

← [结构纪律](/zh/docs/rules/structural-discipline/) | 下一个：[可选安全](/zh/docs/rules/optional-safety/) →

---

### BSK-E0060 — 隐式 `int` → `float` 强制转换

在没有显式转换的情况下将整数传递到期望浮点数的地方。

```python
def area(radius: float) -> float:
    return 3.14159 * radius * radius

area(5)         # 错误——int 作为 float 传递
area(5.0)       # 正确
area(float(5))  # 也正确
```

---

### BSK-E0061 — 隐式 `bool` → `int` 强制转换

在没有显式转换的情况下在算术上下文中使用布尔值。

---

### BSK-E0062 — 隐式 `bytes` → `str` 强制转换

在没有显式解码的情况下交替使用字节和字符串。

---

### BSK-E0063 — 隐式数字扩宽

整数被隐式扩宽为更大的数字类型。
