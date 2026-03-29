from typing import Protocol


class MyProto(Protocol):
    x: int

    def __init__(self) -> None:
        self.y = 0  # E0097 — `y` is not declared in the Protocol
