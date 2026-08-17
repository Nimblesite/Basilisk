from __future__ import annotations


class Animal:
    def speak(self) -> str:
        return ""


class Dog(Animal):
    def speak(self) -> str:
        return "woof"
