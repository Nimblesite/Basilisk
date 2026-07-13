# A realistic file with a mix of typed and untyped code.
# Run: basilisk check examples/mixed.py
#
# The genuine type error is an error out of the box. The untyped parts only
# surface once the opt-in strictness rules are enabled — this repository
# enables them for `examples/**` as warnings in the root `pyproject.toml`.

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


compute(2, "three")  # error[calls_argument_type]: `y` expects `int`, got a `str`
