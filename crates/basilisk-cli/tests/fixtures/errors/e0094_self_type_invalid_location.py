from typing import Self

def foo(bar: Self) -> Self: ...  # E0094 — not within a class
bar: Self  # E0094 — module-level Self
