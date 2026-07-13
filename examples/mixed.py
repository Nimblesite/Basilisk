# A realistic file with a mix of typed and untyped code.
# Run: basilisk check examples/mixed.py
#
# Basilisk will flag the untyped parts and leave the rest alone.

from typing import Optional


def fetch_user(user_id: int) -> Optional[str]:
    # pretend DB lookup
    return None


def save_record(data):  # BSK-E0001: data untyped
    pass  # BSK-E0002: no return type


class Config:
    debug: bool
    timeout: int

    def __init__(self, debug: bool, timeout: int) -> None:
        self.debug = debug
        self.timeout = timeout

    def reset(self):  # BSK-E0002: no return type
        self.debug = False
        self.timeout = 30


def compute(x: int, y: int) -> int:
    return x * y
