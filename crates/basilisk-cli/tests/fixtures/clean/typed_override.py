from __future__ import annotations

from typing import override


class Animal:
    def speak(self: Animal) -> str:
        return ""

    def name(self: Animal) -> str:
        return "animal"


class Dog(Animal):
    @override
    def speak(self: Dog) -> str:
        return "woof"

    def fetch(self: Dog, item: str) -> str:
        return f"fetched {item}"
