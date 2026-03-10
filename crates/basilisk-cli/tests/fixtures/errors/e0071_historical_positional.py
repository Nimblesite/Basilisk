def f1(__x: int) -> None: ...

f1(__x=3)  # E0071 — __x is positional-only

def f2(x: int, __y: int) -> None: ...  # E0071 — __y after positional-or-keyword x
