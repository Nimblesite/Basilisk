from __future__ import annotations


class Config:
    # Annotations without defaults: pure type declarations, no W0050
    host: str
    port: int
    debug: bool


class Point:
    # Annotations without defaults: no W0050
    x: float
    y: float
