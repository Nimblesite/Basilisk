from typing import Generic, TypeVar, Self

T = TypeVar("T")

class Class1(Generic[T]):
    def __new__(cls, x: T) -> Self:
        return super().__new__(cls)

Class1[int](1.0)  # E0074 — float is not compatible with int
