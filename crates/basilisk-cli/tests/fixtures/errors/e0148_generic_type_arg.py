from typing import TypeVar

AnyStr = TypeVar("AnyStr", str, bytes)


def concat(x: AnyStr, y: AnyStr) -> AnyStr:
    return x + y


def bad(s: str, b: bytes) -> None:
    concat(s, b)  # E: constraint groups do not match
