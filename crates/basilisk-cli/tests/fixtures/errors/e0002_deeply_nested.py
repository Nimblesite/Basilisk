def outer(x: int) -> int:
    def middle(y: int) -> int:
        def inner(z: int) -> None:
            return x + y + z
        return inner(0)
    return middle(0)
