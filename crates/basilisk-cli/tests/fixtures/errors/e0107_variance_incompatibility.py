from typing import Generic, TypeVar

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)


class Base(Generic[T]):
    pass


class Bad(Base[T_co]):  # E: invariant param gets covariant arg
    pass
