from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")


def fun(x: T) -> list[T]:
    z: list[S] = []  # E: S is not bound in this function
    return [x]
