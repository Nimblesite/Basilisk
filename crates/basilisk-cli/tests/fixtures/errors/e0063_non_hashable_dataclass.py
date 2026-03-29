from dataclasses import dataclass
from typing import Hashable


@dataclass
class DC1:
    a: int


v: Hashable = DC1(0)  # E0063 — DC1.__hash__ is None


@dataclass(eq=True, frozen=True)
class DC2:
    a: int


v2: Hashable = DC2(0)  # OK — frozen dataclasses are hashable
