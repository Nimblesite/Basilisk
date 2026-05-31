def outer(x: int) -> int:
    def inner(y) -> int:
        return x + y

    return inner(1)
