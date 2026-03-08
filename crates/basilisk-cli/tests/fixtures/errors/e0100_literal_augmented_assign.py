from typing import Literal

def func(a: Literal[3, 4, 5]) -> None:
    a += 3  # E0100 — augmented assign widens Literal type
