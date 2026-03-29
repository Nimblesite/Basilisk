from typing import Self, TypeVar, Generic
from dataclasses import dataclass

T = TypeVar("T")


@dataclass
class LinkedList(Generic[T]):
    value: T
    next: Self | None = None


@dataclass
class OrdinalLinkedList(LinkedList[int]):
    def ordinal_value(self) -> str:
        return str(self.value)


xs = OrdinalLinkedList(value=1, next=LinkedList[int](value=2))  # E0075
