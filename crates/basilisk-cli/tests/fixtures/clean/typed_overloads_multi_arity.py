from __future__ import annotations

from typing import overload


@overload
def resize(x: int) -> str: ...


@overload
def resize(x: int, y: int) -> str: ...


def resize(*args: int) -> str:
    return str(args[0])
