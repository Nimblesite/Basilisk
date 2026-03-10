from typing import TypeVar, TypeVarTuple

T = TypeVar("T")
Ts = TypeVarTuple("Ts")

IntTupleGeneric = tuple[int, T]

IntTupleGeneric[*Ts]  # E: Ts is a TypeVarTuple, not a TypeVar
