from __future__ import annotations


class Config:
    host: str = "localhost"
    port: int = 8080
    debug: bool = False


class Point:
    x: float = 0.0
    y: float = 0.0
