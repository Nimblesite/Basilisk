from typing import Callable


def make_adder(n: int) -> Callable[[int], int]:
    def adder(x: int) -> int:
        return x + n
    return adder


def apply_twice(f: Callable[[int], int], x: int) -> int:
    return f(f(x))


def compose(f: Callable[[int], int], g: Callable[[int], int]) -> Callable[[int], int]:
    def composed(x: int) -> int:
        return f(g(x))
    return composed


add5: Callable[[int], int] = make_adder(5)
print(add5(10))
print(add5(0))

print(apply_twice(make_adder(3), 10))

double: Callable[[int], int] = make_adder(0)  # identity-ish, just for compose demo
add10: Callable[[int], int] = compose(make_adder(7), make_adder(3))
print(add10(100))
