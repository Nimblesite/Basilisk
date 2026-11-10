"""Narrowing inside conditional expressions.

The typing spec's narrowing chapter
(https://typing.python.org/en/latest/spec/narrowing.html) applies
`x is [not] None` guards to the arms of a conditional expression: the arm
where the guard holds sees the narrowed type. The first function is fully
legal; the second returns `None` from its narrowed arm and must be flagged.
"""

from typing import Optional


def coerce(value: Optional[int]) -> int:
    return value if value is not None else 0


def inverted(value: Optional[int]) -> int: return value if value is None else 0  # E
