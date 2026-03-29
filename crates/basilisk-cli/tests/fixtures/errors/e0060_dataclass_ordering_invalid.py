from dataclasses import dataclass


@dataclass(order=True)
class DC1:
    a: str


@dataclass(order=True)
class DC2:
    a: str


dc1 = DC1("x")
dc2 = DC2("y")

if dc1 < dc2:
    pass
