from typing import overload


@overload
def process(x) -> int: ...


@overload
def process(x) -> str: ...


def process(x: int) -> int:
    return x
