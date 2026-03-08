from typing import Protocol, TypeVar

T = TypeVar("T")


class MyProto(Protocol[T]):  # E: T should be covariant
    def method(self) -> T: ...
