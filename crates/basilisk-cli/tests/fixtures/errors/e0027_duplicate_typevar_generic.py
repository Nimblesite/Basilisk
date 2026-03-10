from typing import TypeVar, Generic
T = TypeVar('T')
class Box(Generic[T, T]):
    pass
