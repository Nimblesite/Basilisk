from typing import Sized, TypeVar

ST = TypeVar("ST", bound=Sized)

def longer(x: ST, y: ST) -> ST:
    if len(x) > len(y):
        return x
    return y

longer(3, 3)  # E0080 — int does not implement Sized (__len__)
