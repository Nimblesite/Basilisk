from typing import TypeVar

T = TypeVar("T")


class Node:
    pass


class SymbolTable(dict[str, list[Node]]):
    pass


def takes(x: dict[str, list[object]]) -> None:
    pass


def test(s: SymbolTable) -> None:
    takes(s)  # E: list is invariant, list[Node] != list[object]
