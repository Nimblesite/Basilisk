from typing import overload

class Bytes:
    @overload
    def __getitem__(self, __i: int) -> int: ...
    @overload
    def __getitem__(self, __s: slice) -> bytes: ...
    def __getitem__(self, __i_or_s: int | slice) -> int | bytes: ...

b = Bytes()
b[""]  # E0072 — no overload of __getitem__ accepts str
