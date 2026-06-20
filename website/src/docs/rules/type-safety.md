---
layout: layouts/docs.njk
title: Type Safety — E0010–E0029
description: "Basilisk type-safety rules BSK-E0010 to E0029 — argument mismatches, return type errors, incompatible overrides, unhashable keys, and non-exhaustive matches."
keywords: basilisk, type safety, type mismatch, BSK-E0012, BSK-E0013, BSK-E0016
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Type Safety
  parent: Rules
  order: 2
---

# Type Safety — E0010–E0029

Rules that catch type mismatches, incorrect annotations, and unsound type usage. For the complete, generated list of every code, see the [rules overview](/docs/rules/).

← [Missing Annotations](/docs/rules/missing-annotations/) | [All Rules](/docs/rules/) →

---

### BSK-E0010 — Unresolved import

An `import` refers to a module that cannot be resolved on the configured search paths.

```python
# Error — module cannot be found
from legacy_module import process_data

# Fix — install the package / add it to the workspace, or point
# stub-paths at a .pyi for it
```

Real `basilisk check` output:

![basilisk check output reporting BSK-E0010 for an unresolved import](/assets/images/e0010.png)

---

### BSK-E0011 — Explicit `Any` / return type mismatch

Two checks share this code. An explicit `Any` annotation silences type checking and must be justified; and a returned value that is clearly incompatible with the declared return type is reported.

```python
from typing import Any

# Warning — explicit `Any` must carry a reason
def handle(data: Any) -> bool:
    return True
```

Real `basilisk check` output:

![basilisk check output reporting BSK-E0011 for an explicit Any annotation](/assets/images/e0011.png)

---

### BSK-E0012 — Argument type mismatch

A function is called with an argument of the wrong type.

```python
def greet(name: str) -> str:
    return f"Hello, {name}"

greet(42)  # Error — int is not str
```

Real `basilisk check` output:

![basilisk check output reporting BSK-E0012 for an argument type mismatch](/assets/images/e0012.png)

---

### BSK-E0013 — Return type mismatch

The type of a returned value does not match the declared return type.

```python
def get_count() -> int:
    return "many"  # Error — str is not int
```

Real `basilisk check` output:

![basilisk check output reporting BSK-E0013 for a return type mismatch](/assets/images/e0013.png)

---

### BSK-E0014 — Assignment incompatibility

A value of the wrong type is assigned to an annotated variable.

```python
count: int = "zero"  # Error — str is not int
```

Real `basilisk check` output:

![basilisk check output reporting BSK-E0014 for an assignment incompatibility](/assets/images/e0014.png)

---

### BSK-E0015 — Invalid type argument count

A generic type is used with the wrong number of type arguments.

```python
x: dict[str] = {}   # Error — dict requires 2 type args
```

Real `basilisk check` output:

![basilisk check output reporting BSK-E0015 for an invalid type argument count](/assets/images/e0015.png)

---

### BSK-E0016 — Incompatible method override

An overridden method in a subclass has an incompatible signature.

```python
from typing import override

class Base:
    def process(self, data: str) -> str:
        return data

class Child(Base):
    @override
    def process(self, data: int) -> str:  # Error — parameter type changed
        return str(data)
```

Real `basilisk check` output:

![basilisk check output reporting BSK-E0016 for an incompatible method override](/assets/images/e0016.png)

---

### BSK-E0017 — Incompatible variable override

A class variable is overridden with an incompatible type in a subclass.

---

### BSK-E0018 — Undefined variable

A name is used that has not been defined in the current scope.

```python
def f() -> int:
    return missing_local  # Error — name is not defined
```

Real `basilisk check` output:

![basilisk check output reporting BSK-E0018 for an undefined variable](/assets/images/e0018.png)

---

### BSK-E0019 — Unbound variable

A variable is used before it has been assigned a value in all code paths.

```python
def check(flag: bool) -> str:
    if flag:
        result = "yes"
    return result  # Error — result may be unbound
```

Real `basilisk check` output:

![basilisk check output reporting BSK-E0019 for an unbound variable](/assets/images/e0019.png)

---

### BSK-E0020 — Missing overload implementation

An `@overload` group has no concrete implementation function.

---

### BSK-E0021 — Overlapping overloads

Two `@overload` signatures are indistinguishable from the caller's perspective.

---

### BSK-E0022 — Unhashable type in hash context

A mutable type (like `list`) is used as a dictionary key or set element.

```python
d: dict[list[int], str] = {}  # Error — list is not hashable
```

---

### BSK-E0023 — Non-exhaustive pattern match

A `match` statement does not cover all possible cases for the matched type.

```python
def classify(x: int | str) -> str:
    match x:
        case int():
            return "number"
    # Error — str case not handled
```

---

### BSK-E0024 — Invalid type form

A value that is not a valid type is used in a type position — for example a numeric literal as an annotation.

```python
x: 42 = 0   # Error — `42` is not a type
y: int = 0  # Correct
```

---

### BSK-E0025 — Missing `@override` decorator

A method that overrides a parent class method is missing the `@override` decorator (PEP 698).

```python
class Base:
    def process(self) -> str:
        return "base"

class Child(Base):
    def process(self) -> str:  # Error — missing @override
        return "child"
```

Real `basilisk check` output:

![basilisk check output reporting BSK-E0025 for a missing @override decorator](/assets/images/e0025.png)

---

### BSK-E0026 — `TypeVar` with a single constraint

A `TypeVar` declared with exactly one constraint is meaningless — constraints require two or more.

```python
from typing import TypeVar

T = TypeVar("T", int)        # Error — a single constraint
U = TypeVar("U", int, str)   # Correct — two or more
```

---

### BSK-E0027 — Duplicate `TypeVar` in a `Generic[...]` base

The same `TypeVar` appears more than once in a `Generic[...]` (or `Protocol[...]`) base.

```python
from typing import Generic, TypeVar

T = TypeVar("T")
class Box(Generic[T, T]):  # Error — `T` listed twice
    ...
```

---

### BSK-E0029 — Method defined inside a `TypedDict`

`TypedDict` classes describe data shape only; they may not define methods.

```python
from typing import TypedDict

class Movie(TypedDict):
    title: str
    def play(self) -> None:  # Error — methods aren't allowed in a TypedDict
        ...
```
