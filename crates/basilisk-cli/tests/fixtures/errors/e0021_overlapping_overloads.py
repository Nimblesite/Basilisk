from typing import overload


@overload
def process(x: Any) -> int: ...


@overload
def process(x: Any) -> str: ...


def process(x: int) -> int:
    return x
