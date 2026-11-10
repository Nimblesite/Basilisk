"""TypedDict consistency through inheritance (PEP 728 extra items).

The typing spec's TypedDict chapter
(https://typing.python.org/en/latest/spec/typeddict.html#extra-items) allows
a TypedDict with `extra_items` to be consistent with `dict[str, VT]` when
every field (including inherited and not-required ones) is consistent with
`VT`. Everything below is legal: any diagnostic is a false positive.
"""

from typing_extensions import NotRequired, TypedDict


class IntDict(TypedDict, extra_items=int):
    pass


class IntDictWithNum(IntDict):
    num: NotRequired[int]


def clear_intdict(x: IntDict) -> None:
    v: dict[str, int] = x
    v.clear()


not_required_num_dict: IntDictWithNum = {"num": 1, "bar": 2}
regular_dict: dict[str, int] = not_required_num_dict
clear_intdict(not_required_num_dict)
