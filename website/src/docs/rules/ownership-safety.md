---
layout: layouts/docs.njk
title: Ownership Safety — E0030–E0035
description: "Mojo-inspired ownership analysis rules in Basilisk — mutation of Borrowed parameters, use-after-move, implicit copies, and missing ownership annotations. BSK-E0030 through E0035."
keywords: basilisk, ownership, borrowed, owned, inout, BSK-E0030, BSK-E0031
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Ownership Safety
  parent: Rules
  order: 3
---

# Ownership Safety — E0030–E0035

Mojo-inspired ownership analysis. See [Mojo-Style Safety](/docs/mojo-safety/) for full documentation.

← [Type Safety](/docs/rules/type-safety/) | Next: [Immutability](/docs/rules/immutability/) →

---

### BSK-E0030 — Mutation of `Borrowed` parameter

A `Borrowed` parameter (read-only reference) is mutated.

```python
from typing import Annotated
from basilisk import Borrowed

def summarise(items: Annotated[list[int], Borrowed]) -> int:
    items.append(99)  # Error — cannot mutate Borrowed
    return sum(items)
```

---

### BSK-E0031 — Use after ownership transfer

A value is used after its ownership has been transferred to another binding or function.

```python
from typing import Annotated
from basilisk import Owned

def consume(data: Annotated[list[int], Owned]) -> int:
    return sum(data)

items = [1, 2, 3]
total = consume(items)
items.append(4)  # Error — items was moved into consume()
```

---

### BSK-E0032 — Implicit copy of large structure

A large structure (over the configured size threshold) is implicitly copied. Requires explicit `.copy()` or `Owned` transfer.

---

### BSK-E0033 — Missing ownership annotation (warning promoted to error)

A function parameter that receives a mutable type has no ownership annotation. Basilisk cannot determine the intended contract.

---

### BSK-E0034 — Owned value not consumed or returned

An `Owned` value is created but neither consumed nor returned, leaking the resource.

---

### BSK-E0035 — Multiple mutable references

Two `InOut` references to the same value exist simultaneously.
