from typing import Generator


class A:
    pass


def bad() -> Generator[A, None, None]:
    yield 3  # E: incompatible yield type (int vs A)
