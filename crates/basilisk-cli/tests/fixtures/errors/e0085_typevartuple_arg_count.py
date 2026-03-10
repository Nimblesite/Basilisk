from typing import Generic, TypeVarTuple

Ts = TypeVarTuple("Ts")

class Height:
    def __init__(self, v: int) -> None: ...

class Width:
    def __init__(self, v: int) -> None: ...

class Array(Generic[*Ts]):
    def __init__(self, shape: tuple[*Ts]) -> None: ...

a = Array[Height, Width](Height(1))  # E0085 — expected 2 arguments, got 1
