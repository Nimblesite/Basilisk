from __future__ import annotations

from typing import Optional


def find(haystack: str, needle: str) -> Optional[int]:
    idx = haystack.find(needle)
    return idx if idx >= 0 else None


def coerce(value: Optional[int]) -> int:
    return value if value is not None else 0


def chain(a: Optional[str], b: Optional[str]) -> Optional[str]:
    if a is None or b is None:
        return None
    return a + b
