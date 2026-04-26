---
layout: layouts/docs.njk
title: Optional Safety — E0070–E0073
description: "Basilisk optional safety rules — attribute access on Optional without None check, unsafe method calls, missing guard clauses, and None propagation errors. BSK-E0070 through E0073."
keywords: basilisk, optional, none, nullable, BSK-E0070, BSK-E0073
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
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
