---
layout: layouts/docs.njk
title: Missing Annotations — E0001–E0009
description: "Basilisk rules that flag missing type annotations — unannotated parameters, return types, variables, *args/**kwargs, and class attributes. BSK-0001 through E0009."
keywords: basilisk, missing annotations, type annotations, BSK-0001, BSK-0002
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Missing Annotations
  parent: Rules
  order: 1
---

# Missing Annotations — E0001–E0009

Rules that flag code where type information is absent.

← [All Rules](/docs/rules/) | Next: [Type Safety](/docs/rules/type-safety/) →

---

### BSK-0001 — Missing parameter type annotation

Every function parameter must have an explicit type annotation.

```python
# Error
def process(data) -> str:
    return data.upper()

# Correct
def process(data: str) -> str:
    return data.upper()
```

Real `basilisk check` output:

![basilisk check output reporting BSK-0001 for an unannotated parameter](/assets/images/e0001.png)

---

### BSK-0002 — Missing return type annotation

Every function must declare its return type.

```python
# Error
def get_user(user_id: int):
    return {"id": user_id}

# Correct
def get_user(user_id: int) -> dict[str, int]:
    return {"id": user_id}
```

Real `basilisk check` output:

![basilisk check output reporting BSK-0002 for a missing return type](/assets/images/e0002.png)

---

### BSK-0003 — Missing variable type annotation

A module-level variable whose type cannot be inferred — for example an empty collection — must carry an explicit annotation.

```python
# Error — element type cannot be inferred from an empty list
data = []

# Correct
data: list[str] = []
```

Real `basilisk check` output:

![basilisk check output reporting BSK-0003 for an unannotated empty list](/assets/images/e0003.png)

---

### BSK-0004 — Missing `*args` or `**kwargs` annotation

Variadic arguments must be annotated.

```python
# Error
def log(*args, **kwargs) -> None:
    print(args, kwargs)

# Correct
def log(*args: str, **kwargs: int) -> None:
    print(args, kwargs)
```

Real `basilisk check` output:

![basilisk check output reporting BSK-0004 for unannotated *args and **kwargs](/assets/images/e0004.png)

---

### BSK-0005 — Missing class attribute annotation

A class attribute whose type cannot be inferred — for example an empty collection — must be explicitly annotated.

```python
# Error — element type cannot be inferred from an empty list
class Registry:
    entries = []

# Correct
class Registry:
    entries: list[str] = []
```

Real `basilisk check` output:

![basilisk check output reporting BSK-0005 for an unannotated class attribute](/assets/images/e0005.png)
