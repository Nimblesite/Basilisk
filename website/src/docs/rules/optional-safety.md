---
layout: layouts/docs.njk
title: Optional Safety — E0070–E0073
description: Rules that prevent unsafe access on Optional values.
keywords: basilisk, optional, none, nullable, BSK-E0070, BSK-E0073
eleventyNavigation:
  key: Optional Safety
  parent: Rules
  order: 7
---

# Optional Safety — E0070–E0073

← [Coercion Safety](/docs/rules/coercion-safety/) | Next: [Unused Code](/docs/rules/unused-code/) →

---

### BSK-E0070 — Attribute access on `Optional`

A method or attribute is accessed on a value that may be `None`.

```python
def get_name(user: User | None) -> str:
    return user.name  # Error — user may be None
```

---

### BSK-E0071 — `Optional` passed where non-`Optional` expected

---

### BSK-E0072 — `Optional` returned where non-`Optional` declared

---

### BSK-E0073 — Comparison with `None` without narrowing
