from typing import Callable


def takes_two(a: int, b: str) -> bool:
    return True


def takes_one(a: int) -> bool:
    return True


x: Callable[[int, str], bool] = takes_one  # E: signature mismatch
