from enum import Enum
from typing import Literal


class Color(Enum):
    RED = 1


def func2(a: Literal[Color.RED]) -> None:
    x1: Literal["Color.RED"] = a  # E0068 — string literal != enum member
