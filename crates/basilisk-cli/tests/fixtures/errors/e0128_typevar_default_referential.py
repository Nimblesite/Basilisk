from typing import TypeVar, Generic

S1 = TypeVar("S1")
S2 = TypeVar("S2", default=S1)

Start2T = TypeVar("Start2T", default="StopT")
Stop2T = TypeVar("Stop2T", default=int)


class slice2(Generic[Start2T, Stop2T]):  # E: bad ordering
    pass
