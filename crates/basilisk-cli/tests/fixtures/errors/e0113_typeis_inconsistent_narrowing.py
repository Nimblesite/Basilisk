from typing import TypeIs


def bad_narrowing(val: int) -> TypeIs[str]:  # E: str is not consistent with int
    return isinstance(val, str)
