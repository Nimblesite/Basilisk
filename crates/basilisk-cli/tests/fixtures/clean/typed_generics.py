from __future__ import annotations

from typing import Optional


def first(items: list[int]) -> Optional[int]:
    return items[0] if items else None


def zip_lists(a: list[str], b: list[int]) -> list[tuple[str, int]]:
    return list(zip(a, b))


def flatten(matrix: list[list[float]]) -> list[float]:
    return [x for row in matrix for x in row]


def lookup(table: dict[str, int], key: str) -> Optional[int]:
    return table.get(key)
