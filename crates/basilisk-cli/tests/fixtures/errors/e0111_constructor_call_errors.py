from typing import Generic, TypeVar

T = TypeVar("T")


class MyClass(Generic[T]):
    def __init__(self, x: T) -> None:
        self.x = x


MyClass[int](1.0)  # E: float is not int


class NoInit:
    pass


NoInit(42)  # E: no custom __init__ with arguments
