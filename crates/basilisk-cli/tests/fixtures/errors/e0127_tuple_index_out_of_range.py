from typing import Literal


def f(v: tuple[int, str, list[bool]], b: Literal[5]) -> None:
    v[b]  # E: index 5 out of range for 3-element tuple
    v[4]  # E: index 4 out of range
    v[-4]  # E: index -4 out of range
