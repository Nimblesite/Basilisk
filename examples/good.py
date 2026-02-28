# These all pass Basilisk cleanly.
# Run: cargo run -- check examples/good.py

def greet(name: str) -> str:
    return "Hello " + name


def add(a: int, b: int) -> int:
    return a + b


class User:
    name: str
    age: int

    def __init__(self, name: str, age: int) -> None:
        self.name = name
        self.age = age

    def birthday(self) -> None:
        self.age += 1


def process(*args: str, **kwargs: int) -> None:
    pass
