from typing import TypeVar, Generic

T1 = TypeVar("T1")
T2 = TypeVar("T2")


class Grandparent(Generic[T1, T2]):
    pass


class Parent(Grandparent[T1, T2]):
    pass


class BadChild(Parent[T1, T2], Grandparent[T2, T1]):  # E: inconsistent ordering
    pass
