from typing import Protocol


class MyProtocol(Protocol):
    timeout: str

    def get_value(self) -> int: ...


import sys

x: MyProtocol = sys  # E0079 — sys may not satisfy MyProtocol
