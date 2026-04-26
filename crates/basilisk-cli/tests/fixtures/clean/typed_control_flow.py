from __future__ import annotations


def classify(n: int) -> str:
    if n < 0:
        def negative() -> str:
            return "negative"
        return negative()
    elif n == 0:
        return "zero"
    else:
        return "positive"


def first_even(numbers: list[int]) -> int:
    for n in numbers:
        def is_even(x: int) -> bool:
            return x % 2 == 0
        if is_even(n):
            return n
    return -1


def safe_divide(a: float, b: float) -> float:
    try:
        def do_divide(x: float, y: float) -> float:
            return x / y
        return do_divide(a, b)
    except ZeroDivisionError:
        return 0.0
