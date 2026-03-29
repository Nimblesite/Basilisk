from typing import NamedTuple


class Point(NamedTuple):
    x: int
    y: int
    units: str = "meters"


p = Point(x=1, y=2, units="inches")
v1: tuple[int, int, str] = p  # OK
v2: tuple[int, int] = p  # E0073 — too few elements (2 vs 3 fields)
v3: tuple[int, str, str] = p  # E0073 — incompatible element type
