from typing import TypeVar, Generic

T = TypeVar("T")


class Outer(Generic[T]):
    class Inner(Generic[T]):  # E: reuses outer class TypeVar
        pass
