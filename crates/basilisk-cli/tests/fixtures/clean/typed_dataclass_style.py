from __future__ import annotations


class Config:
    def __init__(self, host: str, port: int, debug: bool) -> None:
        self.host = host
        self.port = port
        self.debug = debug

    def url(self) -> str:
        return f"http://{self.host}:{self.port}"

    def with_debug(self, enabled: bool) -> Config:
        return Config(self.host, self.port, enabled)


class Rect:
    def __init__(self, width: float, height: float) -> None:
        self.width = width
        self.height = height

    def area(self) -> float:
        return self.width * self.height

    def perimeter(self) -> float:
        return 2 * (self.width + self.height)

    def scale(self, factor: float) -> Rect:
        return Rect(self.width * factor, self.height * factor)
