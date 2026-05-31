from __future__ import annotations


class Base:
    count: int = 0


class Child(Base):
    count: str = "zero"
