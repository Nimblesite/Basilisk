class MyClass:
    def __init__(self, x: int, y: str) -> None:
        self.x = x
        self.y = y


def factory(cls: type[MyClass]) -> MyClass:
    return cls()  # E: missing required arguments x, y
