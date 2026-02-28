from __future__ import annotations


def add(a: int, b: int) -> int:
    return a + b


def greet(name: str) -> str:
    return f"Hello, {name}"


def identity(value: float) -> float:
    return value


class Point:
    def __init__(self: Point, x: float, y: float) -> None:
        self.x = x
        self.y = y

    def distance(self: Point) -> float:
        return (self.x ** 2 + self.y ** 2) ** 0.5

    def scale(self: Point, factor: float) -> Point:
        return Point(self.x * factor, self.y * factor)
