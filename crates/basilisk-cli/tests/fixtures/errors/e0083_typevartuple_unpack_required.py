from typing import Generic, TypeVarTuple

Ts = TypeVarTuple("Ts")

class Cls(Generic[Ts]):  # E0083 — TypeVarTuple must be unpacked with *
    ...

def f(*args: Ts) -> None:  # E0083 — TypeVarTuple must be unpacked with *
    ...
