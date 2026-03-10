from typing import Protocol

class MyProto(Protocol):
    def method(self) -> int: ...

obj = MyProto()  # E0099 — cannot instantiate a Protocol
