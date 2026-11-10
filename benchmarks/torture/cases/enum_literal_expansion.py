"""Enum literal expansion — the #374 equivalence.

The typing spec's enumerations chapter
(https://typing.python.org/en/latest/spec/enums.html#enum-literal-expansion)
says a type checker should treat a complete union of all literal members as
EQUIVALENT to the enum type, in both directions. Everything below is legal:
any diagnostic is a false positive.
"""

import enum
from typing import Literal, assert_type


class Answer(enum.Enum):
    Yes = 1
    No = 2


def to_literal(a: Answer) -> None:
    x: Literal[Answer.Yes, Answer.No] = a
    assert_type(a, Literal[Answer.Yes, Answer.No])
    y: Answer = x
    assert_type(y, Answer)
