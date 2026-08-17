from typing import TypeGuard, TypeIs


class Checker:
    def is_int(self) -> TypeGuard[int]:  # E: no narrowing parameter
        return True

    def is_str(cls) -> TypeIs[str]:  # E: no narrowing parameter
        return True
