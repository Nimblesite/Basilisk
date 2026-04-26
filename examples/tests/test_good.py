"""Tests for examples/good.py — fully typed code."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from good import User, add, greet


def test_greet_returns_greeting() -> None:
    assert greet("Alice") == "Hello Alice"


def test_greet_empty_name() -> None:
    assert greet("") == "Hello "


def test_add_positive_numbers() -> None:
    assert add(2, 3) == 5


def test_add_negative_numbers() -> None:
    assert add(-1, -4) == -5


def test_add_zero() -> None:
    assert add(0, 0) == 0


class TestUser:
    def test_init(self) -> None:
        user = User("Bob", 30)
        assert user.name == "Bob"
        assert user.age == 30

    def test_birthday_increments_age(self) -> None:
        user = User("Carol", 25)
        user.birthday()
        assert user.age == 26

    def test_multiple_birthdays(self) -> None:
        user = User("Dave", 40)
        for _ in range(5):
            user.birthday()
        assert user.age == 45
