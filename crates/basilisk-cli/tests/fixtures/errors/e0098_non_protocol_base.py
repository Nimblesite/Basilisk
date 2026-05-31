from typing import Protocol

class Base:
    x: int = 0

class BadProto(Base, Protocol):  # E0098 — Base is not a Protocol
    def method(self) -> int: ...
