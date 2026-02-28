---
layout: layouts/docs.njk
title: Diagnostic Rules
description: Complete reference for all BSK-E (error) and BSK-W (warning) diagnostic codes.
keywords: basilisk rules, type errors, BSK-E, BSK-W, diagnostic codes
eleventyNavigation:
  key: Rules
  order: 5
---

# Diagnostic Rules

Every Basilisk diagnostic has a unique code in the format `BSK-EXXXX` (error) or `BSK-WXXXX` (warning). This page documents all implemented and planned rules.

All rules are enabled by default. There is no opt-in.

---

## Missing Annotations — E0001–E0009

Rules that flag code where type information is absent.

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

### BSK-E0003 — Unresolvable variable type

A variable is assigned a value whose type cannot be inferred. An explicit annotation is required.

```python
# Error — type of result cannot be determined
result = some_dynamic_function()

# Correct
result: list[str] = some_dynamic_function()
```

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

---

## Type Safety — E0010–E0025

Rules that catch type mismatches, incorrect annotations, and unsound type usage.

### BSK-E0010 — Import from untyped module

Importing from a module with no type stubs produces implicit `Any` for all imported names.

```python
# Error — legacy_module has no stubs
from legacy_module import process_data

# Correct — provide or generate stubs
# basilisk stubs generate legacy_module
```

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

### BSK-E0012 — Argument type mismatch

A function is called with an argument of the wrong type.

```python
def greet(name: str) -> str:
    return f"Hello, {name}"

# Error — int is not str
greet(42)
```

### BSK-E0013 — Return type mismatch

The type of a returned value does not match the declared return type.

```python
def get_count() -> int:
    return "many"  # Error — str is not int
```

### BSK-E0014 — Assignment incompatibility

A value of the wrong type is assigned to an annotated variable.

```python
count: int = 0
count = "zero"  # Error — str is not int
```

### BSK-E0015 — Invalid type argument count

A generic type is used with the wrong number of type arguments.

```python
x: dict[str]        # Error — dict requires 2 type args
y: dict[str, int]   # Correct
```

### BSK-E0016 — Incompatible method override

An overridden method in a subclass has an incompatible signature.

```python
class Base:
    def process(self, data: str) -> str: ...

class Child(Base):
    def process(self, data: int) -> str:  # Error — parameter type changed
        ...
```

### BSK-E0017 — Incompatible variable override

A class variable is overridden with an incompatible type in a subclass.

### BSK-E0018 — Undefined variable

A name is used that has not been defined in the current scope.

### BSK-E0019 — Unbound variable

A variable is used before it has been assigned a value in all code paths.

```python
def check(flag: bool) -> str:
    if flag:
        result = "yes"
    return result  # Error — result may be unbound
```

### BSK-E0020 — Missing overload implementation

An `@overload` group has no concrete implementation function.

### BSK-E0021 — Overlapping overloads

Two `@overload` signatures are indistinguishable from the caller's perspective.

### BSK-E0022 — Unhashable type in hash context

A mutable type (like `list`) is used as a dictionary key or set element.

```python
d: dict[list[int], str] = {}  # Error — list is not hashable
```

### BSK-E0023 — Non-exhaustive pattern match

A `match` statement does not cover all possible cases for the matched type.

```python
def classify(x: int | str) -> str:
    match x:
        case int():
            return "number"
    # Error — str case not handled
```

### BSK-E0024 — Invalid type form

A type annotation uses syntax that is not valid.

```python
x: int | = None  # Error — malformed union
```

### BSK-E0025 — Missing `@override` decorator

A method that overrides a parent class method is missing the `@override` decorator (PEP 698).

```python
class Child(Base):
    def process(self) -> str:  # Error — missing @override
        ...
```

---

## Ownership Safety — E0030–E0035

Mojo-inspired ownership analysis. See [Mojo-Style Safety](/docs/mojo-safety/) for full documentation.

### BSK-E0030 — Mutation of `Borrowed` parameter

A `Borrowed` parameter (read-only reference) is mutated.

```python
from typing import Annotated
from basilisk import Borrowed

def summarise(items: Annotated[list[int], Borrowed]) -> int:
    items.append(99)  # Error — cannot mutate Borrowed
    return sum(items)
```

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

### BSK-E0032 — Implicit copy of large structure

A large structure (over the configured size threshold) is implicitly copied. Requires explicit `.copy()` or `Owned` transfer.

### BSK-E0033 — Missing ownership annotation (warning promoted to error)

A function parameter that receives a mutable type has no ownership annotation. Basilisk cannot determine the intended contract.

### BSK-E0034 — Owned value not consumed or returned

An `Owned` value is created but neither consumed nor returned, leaking the resource.

### BSK-E0035 — Multiple mutable references

Two `InOut` references to the same value exist simultaneously.

---

## Immutability — E0040–E0043

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

### BSK-E0041 — Reassignment of immutable parameter

A parameter is reassigned to a new value within the function body.

### BSK-E0042 — Mutable dataclass (prefer `frozen=True`)

A `@dataclass` is defined without `frozen=True`. Frozen dataclasses are safer and work well with Basilisk's immutability model.

### BSK-E0043 — Mutation of `Final` variable

A variable annotated with `Final` is reassigned.

---

## Structural Discipline — E0050–E0054

### BSK-E0050 — Dynamic attribute on typed class

An attribute is set on a class instance that is not declared in the class body.

```python
class Config:
    host: str

c = Config()
c.port = 8080  # Error — port is not a declared attribute
```

### BSK-E0051 — Missing `__init__`

A class defines instance attributes but has no `__init__` method.

### BSK-E0052 — Missing resource cleanup

A class that opens resources (files, connections) has no `__exit__` method or `close()` implementation.

### BSK-E0053 — Missing `__slots__`

A class could use `__slots__` for memory efficiency but does not. (Warning in strict mode.)

### BSK-E0054 — Sealed class subclassed

A class decorated with `@final` from `typing` is subclassed.

---

## Coercion Safety — E0060–E0063

### BSK-E0060 — Implicit `int` → `float` coercion

An integer is passed where a float is expected without explicit conversion.

```python
def area(radius: float) -> float:
    return 3.14159 * radius * radius

area(5)       # Error — int passed as float
area(5.0)     # Correct
area(float(5))  # Also correct
```

### BSK-E0061 — Implicit `bool` → `int` coercion

A boolean is used in an arithmetic context without explicit conversion.

### BSK-E0062 — Implicit `bytes` → `str` coercion

Bytes and strings are used interchangeably without explicit decoding.

### BSK-E0063 — Implicit numeric widening

An integer is implicitly widened to a larger numeric type.

---

## Optional Safety — E0070–E0073

### BSK-E0070 — Attribute access on `Optional`

A method or attribute is accessed on a value that may be `None`.

```python
def get_name(user: User | None) -> str:
    return user.name  # Error — user may be None
```

### BSK-E0071 — `Optional` passed where non-`Optional` expected

### BSK-E0072 — `Optional` returned where non-`Optional` declared

### BSK-E0073 — Comparison with `None` without narrowing

---

## Unused Code — W0080–W0089

Warnings for code that is defined but never used.

| Code | Description |
|---|---|
| BSK-W0080 | Unused import |
| BSK-W0081 | Unused variable |
| BSK-W0082 | Unused function |
| BSK-W0083 | Unused class |
| BSK-W0084 | Unreachable code after `return` or `raise` |
| BSK-W0085 | Dead branch — condition is always `True` or always `False` |

---

## Code Quality — W0090–W0099

Warnings for patterns that are legal but problematic.

| Code | Description |
|---|---|
| BSK-W0090 | Unnecessary `type: ignore` comment — no error at this location |
| BSK-W0091 | Use of deprecated API |
| BSK-W0092 | `type:` comment instead of annotation syntax (Python 2 style) |
| BSK-W0093 | `assert` statement with side effects — assertions can be disabled |
| BSK-W0094 | Mutable default argument |
| BSK-W0095 | Suppression comment without reason |
