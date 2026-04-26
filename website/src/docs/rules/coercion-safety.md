---
layout: layouts/docs.njk
title: Coercion Safety — E0060–E0063
description: Rules that prevent implicit numeric and type coercions.
keywords: basilisk, coercion, int, float, bool, bytes, BSK-E0060, BSK-E0063
eleventyNavigation:
  key: Coercion Safety
  parent: Rules
  order: 6
---

# Coercion Safety — E0060–E0063

← [Structural Discipline](/docs/rules/structural-discipline/) | Next: [Optional Safety](/docs/rules/optional-safety/) →

---

### BSK-E0060 — Implicit `int` → `float` coercion

An integer is passed where a float is expected without explicit conversion.

```python
def area(radius: float) -> float:
    return 3.14159 * radius * radius

area(5)         # Error — int passed as float
area(5.0)       # Correct
area(float(5))  # Also correct
```

---

### BSK-E0061 — Implicit `bool` → `int` coercion

A boolean is used in an arithmetic context without explicit conversion.

---

### BSK-E0062 — Implicit `bytes` → `str` coercion

Bytes and strings are used interchangeably without explicit decoding.

---

### BSK-E0063 — Implicit numeric widening

An integer is implicitly widened to a larger numeric type.
