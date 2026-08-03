"""PEP 612 `ParamSpec` signature preservation through a decorator.

An identity decorator over `Callable[P, R]` preserves the wrapped function's
full signature (https://peps.python.org/pep-0612/). The valid call is clean;
the two invalid calls are required errors: a `str` argument against the
preserved `int` parameter, and a missing second argument against the
preserved arity. A checker that erases the signature at the decorator
boundary misses both and fails the case.
"""

from typing import Callable


def dec[**P, R](f: Callable[P, R]) -> Callable[P, R]:
    return f


@dec
def add(a: int, b: int) -> int:
    return a + b


ok = add(1, 2)
bad_type = add("1", 2)  # E
bad_arity = add(1)  # E
