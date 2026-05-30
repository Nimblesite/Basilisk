def outer(x: int) -> int:
    def inner(y: Any) -> int:
        return x + y

    return inner(1)
