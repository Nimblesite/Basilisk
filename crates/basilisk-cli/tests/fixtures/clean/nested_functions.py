def outer(x: int) -> int:
    def inner(y: int) -> int:
        return x + y

    return inner(1)


def deep(a: str) -> str:
    def middle(b: str) -> str:
        def innermost(c: str) -> str:
            return a + b + c

        return innermost("z")

    return middle("y")
