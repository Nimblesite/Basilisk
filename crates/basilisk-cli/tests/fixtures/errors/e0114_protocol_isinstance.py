from typing import Protocol, runtime_checkable


class Proto1(Protocol):
    name: str


@runtime_checkable
class Proto2(Protocol):
    name: str

    def method(self) -> int: ...


x: object = object()
isinstance(x, Proto1)  # E: not @runtime_checkable
issubclass(type(x), Proto2)  # E: data protocol in issubclass
