from typing import Callable, TypeVar

T = TypeVar("T")


def func5(x: type[T]) -> None:
    pass


func5(Callable)  # E: Callable is not a class


class A:
    pass


class B:
    pass


class C:
    pass


def func4(x: type[A | B]) -> None:
    pass


func4(C)  # E: C is not A or B
