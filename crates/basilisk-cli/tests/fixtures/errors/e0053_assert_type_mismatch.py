from typing import assert_type


def f(a: int | str) -> None:
    assert_type(a, int)
