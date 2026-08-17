from typing import overload


@overload
def double(x: int) -> int: ...


@overload
def double(x: str) -> str: ...


def helper() -> None:
    pass
