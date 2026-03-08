from typing import Literal


def func(a: Literal[0], b: Literal[False]) -> None:
    x1: Literal[False] = a  # E: int 0 != bool False
    x2: Literal[0] = b  # E: bool False != int 0
