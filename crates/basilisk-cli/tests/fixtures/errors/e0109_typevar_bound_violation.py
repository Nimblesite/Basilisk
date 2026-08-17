from typing import TypeVar

TNum = TypeVar("TNum", bound=int)


def identity(s: TNum) -> TNum:
    return s


def caller(s: str) -> None:
    identity(s)  # E: str is not a subtype of int
