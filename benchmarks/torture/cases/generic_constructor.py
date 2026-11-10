"""Generic constructor and call-site inference — the #290 family.

Calling a generic class solves its type parameters from the constructor
arguments, and a generic function's return type follows from its solved
parameters (https://typing.python.org/en/latest/spec/generics.html — PEP 695
syntax). `dict(a=1)` solves `dict[str, int]` through the keyword-arguments
constructor. Every `assert_type` below is required to hold; a checker that
leaves the parameters unsolved (or guesses wrong) fails the case.
"""

from typing import assert_type


class Box[T]:
    def __init__(self, item: T) -> None:
        self.item = item


def unbox[T](box: Box[T]) -> T:
    return box.item


b = Box(1)
assert_type(b, Box[int])
assert_type(unbox(Box("s")), str)

d = dict(a=1)
assert_type(d, dict[str, int])
