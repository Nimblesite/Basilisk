from dataclasses import dataclass


@dataclass(frozen=True)
class Point:
    x: float


p = Point(1.0)
p.x = 2.0
