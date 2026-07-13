# The fixed counterpart of `bad.py` — every diagnostic addressed, including
# the opt-in strictness rules. Passes Basilisk cleanly at full strictness.
# Run: basilisk check examples/good.py

from typing import override


def greet(name: str) -> str:
    return "Hello, " + name


def get_score() -> int:
    return 42


def add(x: int, y: int) -> int:
    return x + y


class Shape:
    def area(self, scale: float) -> float:
        return scale


class Circle(Shape):
    @override
    def area(self, scale: float) -> float:
        return scale * 3.14


def describe(flag: bool) -> str:
    return "on" if flag else "off"


def classify(value: int | str) -> str:
    match value:
        case int():
            return "number"
        case _:
            return "text"


def process(data: str) -> str:
    return data.upper()


def log_all(*args: str, **kwargs: int) -> None:
    pass


def main() -> None:
    print(greet("world"))
    print(add(get_score(), 2))
    print(describe(flag=True))
    print(classify("basilisk"))


if __name__ == "__main__":
    main()
