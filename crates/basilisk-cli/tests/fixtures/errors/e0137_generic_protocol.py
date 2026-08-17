from typing import Generic, Protocol, TypeVar

T_co = TypeVar("T_co", covariant=True)


class Proto2(Protocol[T_co], Generic[T_co]):  # E: Protocol[T] with Generic[T]
    pass
