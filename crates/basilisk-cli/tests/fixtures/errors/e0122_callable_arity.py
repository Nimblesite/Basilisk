from typing import Callable


def takes_cb(cb: Callable[[int, str], bool]) -> None:
    cb(1)  # E: expected 2 arguments, got 1
    cb(1, "a", 3.0)  # E: expected 2 arguments, got 3
    cb(x=1, y="a")  # E: keyword arguments not allowed
