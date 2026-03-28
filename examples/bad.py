# These all get flagged by Basilisk.
# Run: cargo run -- check examples/bad.py


def greet(name):  # BSK-E0001: name has no type annotation
    return "Hello " + name  # BSK-E0002: no return type


def add(a, b):  # BSK-E0001: a, b have no type annotations
    return a + b  # BSK-E0002: no return type


class User:
    def __init__(self, name, age):  # BSK-E0001: name, age untyped
        self.name = name  # BSK-E0005: attribute has no annotation
        self.age = age  # BSK-E0005: attribute has no annotation

    def birthday(self):  # BSK-E0002: no return type
        self.age += 1


def process(*args, **kwargs):  # BSK-E0004: *args/**kwargs untyped
    pass
