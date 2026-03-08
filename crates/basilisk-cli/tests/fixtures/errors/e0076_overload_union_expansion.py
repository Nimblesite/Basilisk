from typing import overload

@overload
def example(x: int, y: str, z: int) -> str: ...
@overload
def example(x: int, y: int, z: int) -> int: ...
def example(x: int, y: int | str, z: int) -> int | str:
    return 1

def check(v: int | str) -> None:
    example(v, v, 1)  # E0076 — str not assignable to int in any overload
