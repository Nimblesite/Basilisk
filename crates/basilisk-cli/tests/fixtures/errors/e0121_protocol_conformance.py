from typing import Protocol


class P(Protocol):
    def method(self) -> None: ...


class C:
    pass


x: P = C()  # E: C does not implement method
