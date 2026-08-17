from __future__ import annotations

# Widening: int literal declared as float — annotation adds information, no W0050
ratio: float = 42

# Empty containers: element type unknown from empty literal, annotation adds info, no W0050
data: list[int] = []
lookup: dict[str, int] = {}

# Union annotation: inferred None does not match int | None, no W0050
value: int | None = None
