"""Benchmark stress fixture for rule `literals_semantics_2`.

Literal value assignment incompatibility (PEP 586): `Literal[0]` is not
`Literal[False]`, and augmented assignment widens a Literal-typed target
to `int`. Generated file - numbered repeats of 4 rotating variants.
"""

import typing
from typing import Literal
from typing import Literal as L


def fa0(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb0(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc0(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd0(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa1(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb1(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc1(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd1(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa2(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb2(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc2(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd2(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa3(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb3(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc3(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd3(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa4(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb4(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc4(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd4(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa5(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb5(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc5(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd5(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa6(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb6(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc6(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd6(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa7(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb7(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc7(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd7(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa8(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb8(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc8(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd8(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa9(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb9(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc9(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd9(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa10(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb10(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc10(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd10(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa11(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb11(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc11(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd11(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa12(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb12(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc12(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd12(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa13(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb13(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc13(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd13(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa14(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb14(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc14(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd14(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa15(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb15(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc15(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd15(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa16(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb16(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc16(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd16(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa17(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb17(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc17(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd17(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa18(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb18(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc18(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd18(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa19(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb19(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc19(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd19(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa20(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb20(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc20(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd20(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa21(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb21(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc21(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd21(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa22(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb22(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc22(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd22(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa23(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb23(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc23(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd23(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa24(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb24(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc24(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd24(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa25(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb25(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc25(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd25(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa26(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb26(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc26(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd26(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa27(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb27(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc27(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd27(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa28(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb28(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc28(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd28(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa29(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb29(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc29(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd29(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa30(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb30(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc30(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd30(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa31(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb31(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc31(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd31(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa32(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb32(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc32(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd32(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa33(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb33(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc33(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd33(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa34(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb34(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc34(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd34(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa35(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb35(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc35(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd35(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa36(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb36(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc36(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd36(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa37(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb37(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc37(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd37(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa38(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb38(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc38(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd38(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa39(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb39(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc39(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd39(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa40(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb40(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc40(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd40(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa41(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb41(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc41(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd41(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa42(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb42(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc42(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd42(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa43(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb43(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc43(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd43(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa44(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb44(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc44(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd44(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa45(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb45(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc45(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd45(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa46(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb46(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc46(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd46(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa47(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb47(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc47(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd47(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa48(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb48(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc48(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd48(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa49(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb49(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc49(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd49(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa50(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb50(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc50(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd50(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa51(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb51(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc51(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd51(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa52(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb52(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc52(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd52(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa53(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb53(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc53(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd53(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa54(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb54(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc54(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd54(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa55(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb55(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc55(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd55(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa56(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb56(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc56(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd56(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa57(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb57(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc57(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd57(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa58(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb58(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc58(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd58(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa59(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb59(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc59(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd59(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa60(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb60(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc60(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd60(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa61(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb61(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc61(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd61(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa62(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb62(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc62(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd62(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa63(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb63(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc63(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd63(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa64(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb64(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc64(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd64(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa65(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb65(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc65(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd65(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa66(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb66(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc66(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd66(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa67(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb67(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc67(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd67(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa68(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb68(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc68(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd68(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa69(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb69(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc69(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd69(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa70(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb70(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc70(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd70(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"


def fa71(a: typing.Literal[0], b: typing.Literal[False]) -> None:
    ok1: typing.Literal[0] = a
    ok2: typing.Literal[False] = b
    x1: typing.Literal[False] = a  # E: int 0 is not bool False
    x2: typing.Literal[0] = b  # E: bool False is not int 0


def fb71(a: typing.Literal[1], b: typing.Literal[True]) -> None:
    ok1: typing.Literal[1] = a
    ok2: typing.Literal[True] = b
    x1: typing.Literal[True] = a  # E: int 1 is not bool True
    x2: typing.Literal[1] = b  # E: bool True is not int 1


def fc71(a: L[3, 4, 5], b: Literal[7]) -> None:
    ok = a.__add__(3)
    a += 3  # E: result widens to int
    if b > 2:
        b -= 1  # E: result widens to int


def fd71(a: typing.Literal[0x14], b: typing.Literal["red"]) -> None:
    ok: typing.Literal[20] = a
    x1: typing.Literal[21] = a  # E: 0x14 == 20, not 21
    x2: typing.Literal["blue"] = b  # E: "red" is not "blue"
