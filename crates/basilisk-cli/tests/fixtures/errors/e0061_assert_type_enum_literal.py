from enum import Enum
from typing import assert_type, Literal


class Status(Enum):
    ACTIVE = 1
    INACTIVE = 2


def process(status: Status) -> None:
    assert_type(status, Literal[Status.ACTIVE])  # E0061 — redundant narrowing
    assert_type(status, Status)  # OK — correct usage
