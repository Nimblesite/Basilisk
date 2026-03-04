from __future__ import annotations

# Widening: int literal declared as float — annotation adds information, no W0050
ratio: float = 42

# Empty containers: generic type not inferrable from empty literal, no W0050
data: list[int] = []
lookup: dict[str, int] = {}

# Union annotation: inferred None does not match int | None, no W0050
value: int | None = None

# Declarations without values: no RHS to infer from, no W0050
count: int
label: str
enabled: bool
