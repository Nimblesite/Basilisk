from typing import Callable, TypeGuard


def takes_callable_str(f: Callable[[object], str]) -> None:
    pass


def simple_typeguard(val: object) -> TypeGuard[int]:
    return isinstance(val, int)


takes_callable_str(simple_typeguard)  # E: TypeGuard is bool, not str
