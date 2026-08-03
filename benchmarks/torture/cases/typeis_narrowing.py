"""PEP 742 `TypeIs` narrowing, both branches.

PEP 742 (https://peps.python.org/pep-0742/) mandates the asymmetric
narrowing: in the positive branch the argument narrows to the intersection
with the `TypeIs` type; in the negative branch the `TypeIs` type is
SUBTRACTED. Both `assert_type` lines are therefore required to hold — a
checker that narrows only the positive branch (or not at all) reports an
`assert_type` mismatch and fails the case.
"""

from typing import TypeIs, assert_type


def is_str(x: object) -> TypeIs[str]:
    return isinstance(x, str)


def split(x: int | str) -> None:
    if is_str(x):
        assert_type(x, str)
    else:
        assert_type(x, int)
