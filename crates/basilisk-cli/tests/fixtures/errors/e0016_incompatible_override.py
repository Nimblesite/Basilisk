from __future__ import annotations
from typing import override


class Base:
    def process(self, data: str) -> str:
        return data


class Child(Base):
    @override
    def process(self, data: int) -> int:
        return data
