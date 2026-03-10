from typing import Final, NamedTuple

X: Final = "x"
Y: Final = "y"
N = NamedTuple("N", [(X, int), (Y, int)])

N(x=3, y=4)        # OK
N(a=1)             # E0064: unknown field `a`
N(x="", y="")      # E0064: field `x` expects `int` but got `str`
