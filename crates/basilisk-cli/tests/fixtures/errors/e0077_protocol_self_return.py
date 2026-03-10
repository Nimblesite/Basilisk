from typing import Protocol, Self

class ShapeProtocol(Protocol):
    def set_scale(self, scale: float) -> Self: ...

class BadReturn:
    def set_scale(self, scale: float) -> int:
        return 42

def accepts(s: ShapeProtocol) -> None: ...

def main(bad: BadReturn) -> None:
    accepts(bad)  # E0077 — BadReturn.set_scale returns int, not Self
