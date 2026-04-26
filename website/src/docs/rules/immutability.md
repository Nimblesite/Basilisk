---
layout: layouts/docs.njk
title: Immutability — E0040–E0043
description: "Basilisk immutability rules — mutation of non-InOut parameters, reassignment of Final variables, frozen dataclass field modification, and immutable collection operations. BSK-E0040 through E0043."
keywords: basilisk, immutability, final, inout, BSK-E0040, BSK-E0043
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Immutability
  parent: Rules
  order: 4
---

# Immutability — E0040–E0043

← [Ownership Safety](/docs/rules/ownership-safety/) | Next: [Structural Discipline](/docs/rules/structural-discipline/) →

---

### BSK-E0040 — Mutation of immutable parameter

A parameter without an `InOut` annotation is mutated.

```python
def process(items: list[int]) -> None:
    items.append(99)  # Error — items is immutable by default
```

Fix by declaring intent:

```python
from typing import Annotated
from basilisk import InOut

def process(items: Annotated[list[int], InOut]) -> None:
    items.append(99)  # OK — InOut declares mutation intent
```

---

### BSK-E0041 — Reassignment of immutable parameter

A parameter is reassigned to a new value within the function body.

---

### BSK-E0042 — Mutable dataclass (prefer `frozen=True`)

A `@dataclass` is defined without `frozen=True`. Frozen dataclasses are safer and work well with Basilisk's immutability model.

---

### BSK-E0043 — Mutation of `Final` variable

A variable annotated with `Final` is reassigned.
