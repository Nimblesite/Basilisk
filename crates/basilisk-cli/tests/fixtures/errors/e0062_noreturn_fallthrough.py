import sys
from typing import NoReturn

def stop() -> NoReturn:
    raise RuntimeError("no way")

def bad(x: int) -> NoReturn:
    if x != 0:
        sys.exit(1)
