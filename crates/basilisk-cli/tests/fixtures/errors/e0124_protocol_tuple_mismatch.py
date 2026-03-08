from typing import Protocol


class RGB(Protocol):
    rgb: tuple[int, int, int]


class Point(RGB):
    def __init__(self, red: int, green: int, blue: str) -> None:
        self.rgb = red, green, blue  # E: blue must be int not str
