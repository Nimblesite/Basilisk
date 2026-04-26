---
layout: layouts/docs.njk
title: Structural Discipline — E0050–E0054
description: Rules that enforce class structure, slots, and sealed classes.
keywords: basilisk, structural discipline, slots, final, BSK-E0050, BSK-E0054
eleventyNavigation:
  key: Structural Discipline
  parent: Rules
  order: 5
---

# Structural Discipline — E0050–E0054

← [Immutability](/docs/rules/immutability/) | Next: [Coercion Safety](/docs/rules/coercion-safety/) →

---

### BSK-E0050 — Dynamic attribute on typed class

An attribute is set on a class instance that is not declared in the class body.

```python
class Config:
    host: str

c = Config()
c.port = 8080  # Error — port is not a declared attribute
```

---

### BSK-E0051 — Missing `__init__`

A class defines instance attributes but has no `__init__` method.

---

### BSK-E0052 — Missing resource cleanup

A class that opens resources (files, connections) has no `__exit__` method or `close()` implementation.

---

### BSK-E0053 — Missing `__slots__`

A class could use `__slots__` for memory efficiency but does not. (Warning in strict mode.)

---

### BSK-E0054 — Sealed class subclassed

A class decorated with `@final` from `typing` is subclassed.
