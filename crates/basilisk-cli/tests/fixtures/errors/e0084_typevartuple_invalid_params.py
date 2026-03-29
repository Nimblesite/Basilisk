from typing import TypeVarTuple

Ts1 = TypeVarTuple(
    "Ts1", covariant=True
)  # E0084 — TypeVarTuple does not support variance
