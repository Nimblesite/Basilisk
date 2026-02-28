from __future__ import annotations
from typing import override


class Base:
    def process(self: Base, data: str) -> str:
        return data


class Child(Base):
    @override
    def process(self: Child, data: int) -> int:
        return data
