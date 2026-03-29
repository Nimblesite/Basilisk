from typing import TypeVar

T = TypeVar("T")
S = TypeVar("S")


def fun(x: T) -> list[T]:
    z: list[S] = []  # E: S is not bound in this function
    return [x]
