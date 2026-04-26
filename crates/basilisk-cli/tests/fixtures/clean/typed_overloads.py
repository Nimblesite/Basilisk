from __future__ import annotations

from typing import overload


@overload
def double(x: int) -> int: ...


@overload
def double(x: str) -> str: ...


def double(x: int | str) -> int | str:
    if isinstance(x, int):
        return x * 2
    return x + x
