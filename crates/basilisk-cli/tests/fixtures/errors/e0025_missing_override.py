from __future__ import annotations


class Animal:
    def speak(self: Animal) -> str:
        return ""


class Dog(Animal):
    def speak(self: Dog) -> str:
        return "woof"
