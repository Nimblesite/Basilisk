from typing import Never, Any, Generic, TypeVar

T = TypeVar("T")
U = TypeVar("U")

def func(c: list[Never]) -> None:
    v: list[int] = c  # E0070 — list is invariant, list[Never] != list[int]

class ClassC(Generic[T]):
    pass

def func2(x: U) -> ClassC[U]:
    return ClassC[Never]()  # E0070 — ClassC is invariant
