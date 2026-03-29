from __future__ import annotations

from typing import override, Any
from basilisk.safety import Borrowed


class Animal:
    def speak(self) -> str:
        return ""

    def __init__(self, dna: Borrowed[Any]) -> None:
        self.dna = dna  # Shows proper borrowing pattern

    def name(self) -> str:
        return "animal"


class Dog(Animal):
    @override
    def speak(self) -> str:
        return "woof"

    def fetch(self, item: str) -> str:
        return f"fetched {item}"
