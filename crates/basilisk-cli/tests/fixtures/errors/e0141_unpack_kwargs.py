from typing import TypedDict, Unpack


class Options(TypedDict):
    name: str
    age: int


def func(name: str, **kwargs: Unpack[Options]) -> None:  # E: name overlaps with TypedDict key
    pass
