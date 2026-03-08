from typing import Protocol


class Proto(Protocol):
    def meth(self) -> int: ...


class Concrete:
    def meth(self) -> int:
        return 42


def fun(cls: type[Proto]) -> int:
    return cls().meth()


fun(Proto)  # E: Protocol class passed to type[Proto]
