"""Function-local bindings shadow same-named module globals.

Python's scoping rules (https://docs.python.org/3/reference/executionmodel.html#resolution-of-names)
make a name assigned anywhere in a function body local to that function for
its WHOLE body. A checker that reads the module-level declaration for a
shadowed name answers for the wrong symbol. Everything below is legal: any
diagnostic is a false positive.
"""

from typing import assert_type


class Widget:
    pass


value: Widget = Widget()
count: str = "shadowed"


def rebind() -> None:
    value = 3
    assert_type(value, int)
    count = [1, 2, 3]
    assert_type(count, list[int])


def parameter_shadow(value: int, count: list[int]) -> None:
    assert_type(value, int)
    assert_type(count, list[int])
