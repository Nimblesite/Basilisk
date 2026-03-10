from typing import TypeVarTuple, Generic

Ts1 = TypeVarTuple("Ts1")
Ts2 = TypeVarTuple("Ts2")

class Array3(Generic[*Ts1, *Ts2]):  # E0086 — multiple TypeVarTuples not allowed
    ...
