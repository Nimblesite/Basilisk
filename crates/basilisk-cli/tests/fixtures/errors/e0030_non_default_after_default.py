from typing import TypeVar, Generic
T = TypeVar('T', default=int)
S = TypeVar('S')
class Box(Generic[T, S]):
    pass
