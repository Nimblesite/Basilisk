---
layout: layouts/docs.njk
title: Missing Annotations — E0001–E0009
description: Rules that flag code where type information is absent.
keywords: basilisk, missing annotations, type annotations, BSK-E0001, BSK-E0002
eleventyNavigation:
  key: Missing Annotations
  parent: Rules
  order: 1
---

# Missing Annotations — E0001–E0009

Rules that flag code where type information is absent.

← [All Rules](/docs/rules/) | Next: [Type Safety](/docs/rules/type-safety/) →

---

### BSK-E0001 — Missing parameter type annotation

Every function parameter must have an explicit type annotation.

```python
# Error
def process(data):
    return data.upper()

# Correct
def process(data: str) -> str:
    return data.upper()
```

---

### BSK-E0002 — Missing return type annotation

Every function must declare its return type.

```python
# Error
def get_user(user_id: int):
    return {"id": user_id}

# Correct
def get_user(user_id: int) -> dict[str, int]:
    return {"id": user_id}
```

---

### BSK-E0003 — Unresolvable variable type

A variable is assigned a value whose type cannot be inferred. An explicit annotation is required.

```python
# Error — type of result cannot be determined
result = some_dynamic_function()

# Correct
result: list[str] = some_dynamic_function()
```

---

### BSK-E0004 — Missing `*args` or `**kwargs` annotation

Variadic arguments must be annotated.

```python
# Error
def log(*args, **kwargs):
    print(args, kwargs)

# Correct
def log(*args: str, **kwargs: int) -> None:
    print(args, kwargs)
```

---

### BSK-E0005 — Missing class attribute annotation

Class-level attributes must be explicitly annotated.

```python
# Error
class Config:
    host = "localhost"
    port = 8080

# Correct
class Config:
    host: str = "localhost"
    port: int = 8080
```
