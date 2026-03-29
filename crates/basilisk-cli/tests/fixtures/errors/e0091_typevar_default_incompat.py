from typing import TypeVar

Ok1 = TypeVar("Ok1", bound=float, default=int)  # OK — int <: float
Invalid1 = TypeVar("Invalid1", bound=str, default=int)  # E0091 — int is not <: str

Ok2 = TypeVar("Ok2", float, str, default=float)  # OK
Invalid2 = TypeVar(
    "Invalid2", float, str, default=int
)  # E0091 — int not in {float, str}
