from typing import Generic, TypeVar

T1 = TypeVar("T1")
T2 = TypeVar("T2")

class MyGeneric(Generic[T1, T2]): ...

MyGeneric[int]          # E0092 — 1 arg but at least 2 required
MyGeneric[int, str]     # OK
