from typing import Protocol


class Proto(Protocol):
    def meth(self) -> int: ...


class Concrete:
    def meth(self) -> int:
        return 42


def fun(cls: type[Proto]) -> int:
    return cls().meth()


fun(Proto)  # E: Protocol class itself passed to type[Proto]

var: type[Proto]
var = Proto  # E: Protocol class assigned to type[Proto]
var = Concrete  # OK
