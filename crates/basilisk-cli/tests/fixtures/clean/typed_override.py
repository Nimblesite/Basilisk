from __future__ import annotations

from typing import override


class Animal:
    def speak(self) -> str:
        return ""

    def name(self) -> str:
        return "animal"


class Dog(Animal):
    @override
    def speak(self) -> str:
        return "woof"

    def fetch(self, item: str) -> str:
        return f"fetched {item}"
