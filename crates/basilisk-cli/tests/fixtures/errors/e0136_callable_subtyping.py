from typing import Callable


def func(
    cb1: Callable[[float], int],
    cb3: Callable[[int], int],
) -> None:
    f6: Callable[[float], float] = cb3  # E: int param is not supertype of float
