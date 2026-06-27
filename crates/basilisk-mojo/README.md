# basilisk-mojo

Mojo-inspired ownership and immutability analysis for Basilisk.

## Role in Basilisk

This crate implements **static ownership semantics** as type annotations over standard Python syntax. Using `Annotated` from the `typing` module, developers can declare ownership contracts that Basilisk enforces at analysis time — catching mutation of borrowed values and use-after-move before they reach production.

## Key concepts

- **`Borrowed`** — a read-only reference. Mutation is a type error.
- **`InOut`** — a mutable reference. Must be explicitly declared.
- **`Owned`** — ownership is transferred. Use after transfer is a type error.
- **Standard Python syntax** — uses `typing.Annotated`, no compiler or runtime required.
- **Mojo compatibility** — code that passes these checks is structurally compatible with Mojo's type expectations.

## Example

```python
from typing import Annotated
from basilisk import Borrowed

def summarise(items: Annotated[list[int], Borrowed]) -> int:
    items.append(99)  # generics_defaults: mutation of Borrowed parameter
    return sum(items)
```

## Status

Phase 4 — ownership annotations designed, implementation in progress.
