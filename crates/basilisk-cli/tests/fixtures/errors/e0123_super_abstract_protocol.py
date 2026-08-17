from typing import Protocol
from abc import abstractmethod


class PColor(Protocol):
    @abstractmethod
    def draw(self) -> str:
        ...


class BadColor(PColor):
    def draw(self) -> str:
        return super().draw()  # E: no default implementation
