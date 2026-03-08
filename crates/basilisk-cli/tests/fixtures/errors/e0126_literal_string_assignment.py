from typing import Literal, LiteralString


def func(b: Literal["two"], non_literal: str) -> None:
    x1: Literal[""] = b  # E: different literal values
    x2: LiteralString = f"{non_literal}"  # E: non-literal in f-string
