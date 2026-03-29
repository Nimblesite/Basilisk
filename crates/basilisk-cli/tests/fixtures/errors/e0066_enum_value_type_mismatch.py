from enum import Enum


class Color(Enum):
    _value_: int
    RED = 1  # OK — int matches int
    GREEN = "green"  # E0066 — str is not compatible with int
