from __future__ import annotations


class Config:
    def __init__(self: Config, host: str, port: int, debug: bool) -> None:
        self.host = host
        self.port = port
        self.debug = debug

    def url(self: Config) -> str:
        return f"http://{self.host}:{self.port}"

    def with_debug(self: Config, enabled: bool) -> Config:
        return Config(self.host, self.port, enabled)


class Rect:
    def __init__(self: Rect, width: float, height: float) -> None:
        self.width = width
        self.height = height

    def area(self: Rect) -> float:
        return self.width * self.height

    def perimeter(self: Rect) -> float:
        return 2 * (self.width + self.height)

    def scale(self: Rect, factor: float) -> Rect:
        return Rect(self.width * factor, self.height * factor)
