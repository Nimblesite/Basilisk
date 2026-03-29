from typing import TypeVar

K = TypeVar("K")


class ClassA[V](dict[K, V]): ...
