from typing import Generic, TypeVar

T = TypeVar("T")


class Node(Generic[T]):
    label: T


Node[int].label = 1  # E: instance attribute on class
Node.label  # E: instance attribute on class
