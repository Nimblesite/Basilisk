---
layout: layouts/docs.njk
title: "Mojo-Style Ownership and Immutability for Python"
description: "Add Borrowed, InOut, and Owned annotations to Python functions. Basilisk statically verifies ownership semantics, immutability, and coercion safety — no Mojo compiler needed."
keywords: basilisk ownership, borrowed, inout, owned, mojo, python ownership analysis
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Mojo Safety
  order: 8
---

# Mojo-Style Safety Annotations

Basilisk extends Python's type system with ownership semantics inspired by Mojo. These are static analysis annotations — no runtime overhead, no Mojo compiler required — that catch an entire class of bugs that standard type checking misses.

## Why ownership matters in Python

Python's standard type system describes *what* a value is. It does not describe *how* a value may be used. You can annotate a function parameter as `list[int]` and still have callers unexpectedly mutate the list, or have the function hold a reference after appearing to transfer it.

Mojo solved this in its language design by distinguishing between:
- **`borrowed`** — a read-only reference
- **`inout`** — a mutable reference
- **`owned`** — full ownership; the callee may consume the value

Basilisk brings these distinctions to Python as `Annotated` type metadata, statically checked at analysis time.

---

## The three ownership annotations

Import the annotations from `basilisk`:

```python
from typing import Annotated
from basilisk import Borrowed, InOut, Owned
```

### `Borrowed` — read-only reference

The function may read the value but must not modify it. This is the **default** for all function parameters in Basilisk.

```python
def summarise(items: Annotated[list[int], Borrowed]) -> int:
    return sum(items)  # OK — read-only access
```

Attempting to mutate a `Borrowed` parameter is **BSK-E0030**:

```python
def bad_summarise(items: Annotated[list[int], Borrowed]) -> int:
    items.sort()  # error[BSK-E0030]: mutation of Borrowed parameter
    return sum(items)
```

### `InOut` — mutable reference

The function may read and modify the value. The caller retains ownership.

```python
def normalise(values: Annotated[list[float], InOut]) -> None:
    total = sum(values)
    for i, v in enumerate(values):
        values[i] = v / total  # OK — InOut declared
```

### `Owned` — ownership transferred

The function takes ownership of the value. The caller must not use the value after the call.

```python
def into_sorted(items: Annotated[list[int], Owned]) -> list[int]:
    items.sort()
    return items

data = [3, 1, 2]
result = into_sorted(data)
data.append(4)  # error[BSK-E0031]: use after ownership transfer
```

---

## Immutability by default

Basilisk enforces **immutability by default** for all function parameters. Even without an explicit `Borrowed` annotation, parameters that are mutated produce **BSK-E0040**:

```python
def process(items: list[int]) -> list[int]:
    items.append(0)  # error[BSK-E0040]: mutation of immutable parameter
    return items
```

To allow mutation, you must declare it explicitly:

```python
from typing import Annotated
from basilisk import InOut

def process(items: Annotated[list[int], InOut]) -> list[int]:
    items.append(0)  # OK — mutation is explicitly declared
    return items
```

This makes function contracts visible at the call site. When you see a function call, you can tell immediately from the annotation whether the callee will modify the passed value.

---

## Coercion safety

Python performs several implicit numeric conversions that can hide bugs. Basilisk flags all of them:

### BSK-E0060 — `int` → `float`

```python
def area(radius: float) -> float:
    return 3.14159 * radius * radius

area(5)       # error[BSK-E0060]: implicit int→float coercion
area(5.0)     # OK
area(float(5))  # OK — explicit conversion
```

### BSK-E0061 — `bool` → `int`

```python
def count(flags: list[bool]) -> int:
    total: int = 0
    for f in flags:
        total += f  # error[BSK-E0061]: implicit bool→int coercion
    return total

# Correct
total += int(f)
```

### BSK-E0062 — `bytes` → `str`

```python
def log(message: str) -> None:
    print(message)

data: bytes = b"hello"
log(data)  # error[BSK-E0062]: implicit bytes→str coercion
log(data.decode("utf-8"))  # OK
```

### BSK-E0063 — Numeric widening

Implicit widening from smaller to larger numeric types (e.g., `int32` → `int64`) requires explicit conversion.

---

## Frozen dataclasses

Mutable dataclasses are a common source of bugs. Basilisk flags `@dataclass` without `frozen=True` as **BSK-E0042**:

```python
from dataclasses import dataclass

@dataclass
class Point:      # warning[BSK-E0042]: prefer frozen=True
    x: float
    y: float

@dataclass(frozen=True)
class Point:      # OK — immutable by design
    x: float
    y: float
```

---

## Mojo compatibility matrix

If you plan to eventually target Mojo, Basilisk's annotations are structurally compatible. Here is how Mojo concepts map to Basilisk's static analysis:

| Mojo concept | Basilisk annotation | Static check |
|---|---|---|
| `borrowed` parameter | `Annotated[T, Borrowed]` | No mutation allowed (BSK-E0030) |
| `inout` parameter | `Annotated[T, InOut]` | Mutation allowed; caller retains ownership |
| `owned` parameter | `Annotated[T, Owned]` | Caller must not use value after call (BSK-E0031) |
| `fn` function | Any Basilisk-checked function | All parameters must be typed |
| `alias` declaration | `Final[T]` | Mutation is BSK-E0043 |
| `let` binding | `Final` annotation | Reassignment is BSK-E0043 |
| `struct` (value type) | `@dataclass(frozen=True)` | No dynamic attributes (BSK-E0050) |
| No implicit coercion | Coercion rules E0060–E0063 | Explicit conversion required |

Python code that passes Basilisk's ownership and immutability checks is structurally ready for Mojo. The annotations translate directly to Mojo's function signature conventions.

---

## Enabling and disabling

All Mojo safety checks are enabled by default. They can be individually disabled in `pyproject.toml`:

```toml
[tool.basilisk.mojo-safety]
ownership = true           # BSK-E0030–E0035
immutability = true        # BSK-E0040–E0043
no-implicit-coercion = true  # BSK-E0060–E0063
```

To disable a specific rule project-wide:

```toml
[tool.basilisk]
rules.disable = ["BSK-E0042"]  # Don't require frozen=True on dataclasses
```

To disable for a specific path:

```toml
[tool.basilisk.per-path-overrides."legacy/**"]
rules.ignore = ["BSK-E0040", "BSK-E0041"]
```
