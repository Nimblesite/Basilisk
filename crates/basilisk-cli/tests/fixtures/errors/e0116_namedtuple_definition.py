from typing import NamedTuple


class BadTuple(NamedTuple):
    _hidden: int  # E: field name starts with underscore
    normal: str
