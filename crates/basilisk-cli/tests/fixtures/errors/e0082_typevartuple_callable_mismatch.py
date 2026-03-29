from typing import TypeVarTuple, Callable

Ts = TypeVarTuple("Ts")


class Process:
    def __init__(self, target: Callable[[*Ts], None], args: tuple[*Ts]) -> None: ...


def func1(arg1: int, arg2: str) -> None: ...


Process(target=func1, args=(0, ""))  # OK
Process(target=func1, args=("", 0))  # E0082 — str, int does not match int, str
