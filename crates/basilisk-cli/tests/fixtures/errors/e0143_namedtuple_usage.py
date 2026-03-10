from typing import NamedTuple


class Point(NamedTuple):
    x: int
    y: int
    units: str = "meters"


p = Point(1, 2)
p[3]  # E: out-of-bounds index
p.x = 3  # E: NamedTuple fields are read-only
