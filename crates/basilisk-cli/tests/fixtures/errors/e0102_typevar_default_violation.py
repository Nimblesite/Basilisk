from typing import TypeVar

T2 = TypeVar("T2", default=T1)  # E: T1 not defined yet
T1 = TypeVar("T1")
