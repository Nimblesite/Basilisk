from typing import TypeAlias, Union

RecursiveUnion: TypeAlias = Union["RecursiveUnion", int]  # E: cyclical reference
