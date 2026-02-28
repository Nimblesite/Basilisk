from __future__ import annotations


class Animal:
    def speak(self: Animal) -> str:
        return ""

    def name(self: Animal) -> str:
        return "animal"


class Dog(Animal):
    def speak(self: Dog) -> str:
        return "woof"

    def fetch(self: Dog, item: str) -> str:
        return f"fetched {item}"


class Cat(Animal):
    def speak(self: Cat) -> str:
        return "meow"

    def purr(self: Cat, duration: float) -> None:
        pass
