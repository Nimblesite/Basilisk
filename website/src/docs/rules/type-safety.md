---
layout: layouts/docs.njk
title: Type Safety — E0010–E0025
description: Rules that catch type mismatches, incorrect annotations, and unsound type usage.
keywords: basilisk, type safety, type mismatch, BSK-E0012, BSK-E0013, BSK-E0016
eleventyNavigation:
  key: Type Safety
  parent: Rules
  order: 2
---

# Type Safety — E0010–E0025

Rules that catch type mismatches, incorrect annotations, and unsound type usage.

← [Missing Annotations](/docs/rules/missing-annotations/) | Next: [Ownership Safety](/docs/rules/ownership-safety/) →

---

### BSK-E0010 — Import from untyped module

Importing from a module with no type stubs produces implicit `Any` for all imported names.

```python
# Error — legacy_module has no stubs
from legacy_module import process_data

# Correct — provide or generate stubs
# basilisk stubs generate legacy_module
```

---

### BSK-E0011 — Implicit `Any`

`Any` must be explicitly annotated with a suppression reason. Implicit `Any` from inference is not permitted.

```python
# Error
def handle(data: Any) -> bool:
    ...

# Correct (with justification)
def handle(
    data: Any,  # basilisk: ignore[BSK-E0011] -- awaiting stubs for third-party SDK
) -> bool:
    ...
```

---

### BSK-E0012 — Argument type mismatch

A function is called with an argument of the wrong type.

```python
def greet(name: str) -> str:
    return f"Hello, {name}"

# Error — int is not str
greet(42)
```

---

### BSK-E0013 — Return type mismatch

The type of a returned value does not match the declared return type.

```python
def get_count() -> int:
    return "many"  # Error — str is not int
```

---

### BSK-E0014 — Assignment incompatibility

A value of the wrong type is assigned to an annotated variable.

```python
count: int = 0
count = "zero"  # Error — str is not int
```

---

### BSK-E0015 — Invalid type argument count

A generic type is used with the wrong number of type arguments.

```python
x: dict[str]        # Error — dict requires 2 type args
y: dict[str, int]   # Correct
```

---

### BSK-E0016 — Incompatible method override

An overridden method in a subclass has an incompatible signature.

```python
class Base:
    def process(self, data: str) -> str: ...

class Child(Base):
    def process(self, data: int) -> str:  # Error — parameter type changed
        ...
```

---

### BSK-E0017 — Incompatible variable override

A class variable is overridden with an incompatible type in a subclass.

---

### BSK-E0018 — Undefined variable

A name is used that has not been defined in the current scope.

---

### BSK-E0019 — Unbound variable

A variable is used before it has been assigned a value in all code paths.

```python
def check(flag: bool) -> str:
    if flag:
        result = "yes"
    return result  # Error — result may be unbound
```

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

A type annotation uses syntax that is not valid.

```python
x: int | = None  # Error — malformed union
```

---

### BSK-E0025 — Missing `@override` decorator

A method that overrides a parent class method is missing the `@override` decorator (PEP 698).

```python
class Child(Base):
    def process(self) -> str:  # Error — missing @override
        ...
```
