"""Benchmark stress fixture for the `callables_subtyping` rule.

Repeats numbered variants of callable-subtyping violations: assignments of
Callable-typed parameters to Callable-annotated targets where the return
type breaks covariance or a parameter type breaks contravariance (PEP 484).
"""

from typing import Callable


def fn0(
    cb_a0: Callable[[float], int],
    cb_b0: Callable[[float], float],
    cb_c0: Callable[[float], complex],
    cb_d0: Callable[[float, float], int],
) -> None:
    ok_a0: Callable[[int], float] = cb_a0
    ok_b0: Callable[[float], float] = cb_b0
    bad_a0: Callable[[complex], int] = cb_a0
    bad_b0: Callable[[complex], float] = cb_b0
    bad_c0: Callable[[complex, complex], int] = cb_d0
    bad_d0: Callable[[float, complex], int] = cb_d0

def fn1(
    cb_a1: Callable[[float], int],
    cb_b1: Callable[[float], float],
    cb_c1: Callable[[float], complex],
    cb_d1: Callable[[float, float], int],
) -> None:
    ok_a1: Callable[[float], complex] = cb_c1
    ok_b1: Callable[[int], complex] = cb_c1
    bad_a1: Callable[[float], float] = cb_c1
    bad_b1: Callable[[int], float] = cb_c1
    bad_c1: Callable[[complex, float], int] = cb_d1
    bad_d1: Callable[[complex], float] = cb_b1

def fn2(
    cb_a2: Callable[[float], int],
    cb_b2: Callable[[float], float],
    cb_c2: Callable[[float], complex],
    cb_d2: Callable[[float, float], int],
) -> None:
    ok_a2: Callable[[int], int] = cb_a2
    ok_b2: Callable[[int, float], float] = cb_d2
    bad_a2: Callable[[complex], int] = cb_a2
    bad_b2: Callable[[float], float] = cb_c2
    bad_c2: Callable[[float, complex], int] = cb_d2
    bad_d2: Callable[[int], float] = cb_c2

def fn3(
    cb_a3: Callable[[float], int],
    cb_b3: Callable[[float], float],
    cb_c3: Callable[[float], complex],
    cb_d3: Callable[[float, float], int],
) -> None:
    ok_a3: Callable[[int], float] = cb_a3
    ok_b3: Callable[[float], float] = cb_b3
    bad_a3: Callable[[complex], int] = cb_a3
    bad_b3: Callable[[complex], float] = cb_b3
    bad_c3: Callable[[complex, complex], int] = cb_d3
    bad_d3: Callable[[float, complex], int] = cb_d3

def fn4(
    cb_a4: Callable[[float], int],
    cb_b4: Callable[[float], float],
    cb_c4: Callable[[float], complex],
    cb_d4: Callable[[float, float], int],
) -> None:
    ok_a4: Callable[[float], complex] = cb_c4
    ok_b4: Callable[[int], complex] = cb_c4
    bad_a4: Callable[[float], float] = cb_c4
    bad_b4: Callable[[int], float] = cb_c4
    bad_c4: Callable[[complex, float], int] = cb_d4
    bad_d4: Callable[[complex], float] = cb_b4

def fn5(
    cb_a5: Callable[[float], int],
    cb_b5: Callable[[float], float],
    cb_c5: Callable[[float], complex],
    cb_d5: Callable[[float, float], int],
) -> None:
    ok_a5: Callable[[int], int] = cb_a5
    ok_b5: Callable[[int, float], float] = cb_d5
    bad_a5: Callable[[complex], int] = cb_a5
    bad_b5: Callable[[float], float] = cb_c5
    bad_c5: Callable[[float, complex], int] = cb_d5
    bad_d5: Callable[[int], float] = cb_c5

def fn6(
    cb_a6: Callable[[float], int],
    cb_b6: Callable[[float], float],
    cb_c6: Callable[[float], complex],
    cb_d6: Callable[[float, float], int],
) -> None:
    ok_a6: Callable[[int], float] = cb_a6
    ok_b6: Callable[[float], float] = cb_b6
    bad_a6: Callable[[complex], int] = cb_a6
    bad_b6: Callable[[complex], float] = cb_b6
    bad_c6: Callable[[complex, complex], int] = cb_d6
    bad_d6: Callable[[float, complex], int] = cb_d6

def fn7(
    cb_a7: Callable[[float], int],
    cb_b7: Callable[[float], float],
    cb_c7: Callable[[float], complex],
    cb_d7: Callable[[float, float], int],
) -> None:
    ok_a7: Callable[[float], complex] = cb_c7
    ok_b7: Callable[[int], complex] = cb_c7
    bad_a7: Callable[[float], float] = cb_c7
    bad_b7: Callable[[int], float] = cb_c7
    bad_c7: Callable[[complex, float], int] = cb_d7
    bad_d7: Callable[[complex], float] = cb_b7

def fn8(
    cb_a8: Callable[[float], int],
    cb_b8: Callable[[float], float],
    cb_c8: Callable[[float], complex],
    cb_d8: Callable[[float, float], int],
) -> None:
    ok_a8: Callable[[int], int] = cb_a8
    ok_b8: Callable[[int, float], float] = cb_d8
    bad_a8: Callable[[complex], int] = cb_a8
    bad_b8: Callable[[float], float] = cb_c8
    bad_c8: Callable[[float, complex], int] = cb_d8
    bad_d8: Callable[[int], float] = cb_c8

def fn9(
    cb_a9: Callable[[float], int],
    cb_b9: Callable[[float], float],
    cb_c9: Callable[[float], complex],
    cb_d9: Callable[[float, float], int],
) -> None:
    ok_a9: Callable[[int], float] = cb_a9
    ok_b9: Callable[[float], float] = cb_b9
    bad_a9: Callable[[complex], int] = cb_a9
    bad_b9: Callable[[complex], float] = cb_b9
    bad_c9: Callable[[complex, complex], int] = cb_d9
    bad_d9: Callable[[float, complex], int] = cb_d9

def fn10(
    cb_a10: Callable[[float], int],
    cb_b10: Callable[[float], float],
    cb_c10: Callable[[float], complex],
    cb_d10: Callable[[float, float], int],
) -> None:
    ok_a10: Callable[[float], complex] = cb_c10
    ok_b10: Callable[[int], complex] = cb_c10
    bad_a10: Callable[[float], float] = cb_c10
    bad_b10: Callable[[int], float] = cb_c10
    bad_c10: Callable[[complex, float], int] = cb_d10
    bad_d10: Callable[[complex], float] = cb_b10

def fn11(
    cb_a11: Callable[[float], int],
    cb_b11: Callable[[float], float],
    cb_c11: Callable[[float], complex],
    cb_d11: Callable[[float, float], int],
) -> None:
    ok_a11: Callable[[int], int] = cb_a11
    ok_b11: Callable[[int, float], float] = cb_d11
    bad_a11: Callable[[complex], int] = cb_a11
    bad_b11: Callable[[float], float] = cb_c11
    bad_c11: Callable[[float, complex], int] = cb_d11
    bad_d11: Callable[[int], float] = cb_c11

def fn12(
    cb_a12: Callable[[float], int],
    cb_b12: Callable[[float], float],
    cb_c12: Callable[[float], complex],
    cb_d12: Callable[[float, float], int],
) -> None:
    ok_a12: Callable[[int], float] = cb_a12
    ok_b12: Callable[[float], float] = cb_b12
    bad_a12: Callable[[complex], int] = cb_a12
    bad_b12: Callable[[complex], float] = cb_b12
    bad_c12: Callable[[complex, complex], int] = cb_d12
    bad_d12: Callable[[float, complex], int] = cb_d12

def fn13(
    cb_a13: Callable[[float], int],
    cb_b13: Callable[[float], float],
    cb_c13: Callable[[float], complex],
    cb_d13: Callable[[float, float], int],
) -> None:
    ok_a13: Callable[[float], complex] = cb_c13
    ok_b13: Callable[[int], complex] = cb_c13
    bad_a13: Callable[[float], float] = cb_c13
    bad_b13: Callable[[int], float] = cb_c13
    bad_c13: Callable[[complex, float], int] = cb_d13
    bad_d13: Callable[[complex], float] = cb_b13

def fn14(
    cb_a14: Callable[[float], int],
    cb_b14: Callable[[float], float],
    cb_c14: Callable[[float], complex],
    cb_d14: Callable[[float, float], int],
) -> None:
    ok_a14: Callable[[int], int] = cb_a14
    ok_b14: Callable[[int, float], float] = cb_d14
    bad_a14: Callable[[complex], int] = cb_a14
    bad_b14: Callable[[float], float] = cb_c14
    bad_c14: Callable[[float, complex], int] = cb_d14
    bad_d14: Callable[[int], float] = cb_c14

def fn15(
    cb_a15: Callable[[float], int],
    cb_b15: Callable[[float], float],
    cb_c15: Callable[[float], complex],
    cb_d15: Callable[[float, float], int],
) -> None:
    ok_a15: Callable[[int], float] = cb_a15
    ok_b15: Callable[[float], float] = cb_b15
    bad_a15: Callable[[complex], int] = cb_a15
    bad_b15: Callable[[complex], float] = cb_b15
    bad_c15: Callable[[complex, complex], int] = cb_d15
    bad_d15: Callable[[float, complex], int] = cb_d15

def fn16(
    cb_a16: Callable[[float], int],
    cb_b16: Callable[[float], float],
    cb_c16: Callable[[float], complex],
    cb_d16: Callable[[float, float], int],
) -> None:
    ok_a16: Callable[[float], complex] = cb_c16
    ok_b16: Callable[[int], complex] = cb_c16
    bad_a16: Callable[[float], float] = cb_c16
    bad_b16: Callable[[int], float] = cb_c16
    bad_c16: Callable[[complex, float], int] = cb_d16
    bad_d16: Callable[[complex], float] = cb_b16

def fn17(
    cb_a17: Callable[[float], int],
    cb_b17: Callable[[float], float],
    cb_c17: Callable[[float], complex],
    cb_d17: Callable[[float, float], int],
) -> None:
    ok_a17: Callable[[int], int] = cb_a17
    ok_b17: Callable[[int, float], float] = cb_d17
    bad_a17: Callable[[complex], int] = cb_a17
    bad_b17: Callable[[float], float] = cb_c17
    bad_c17: Callable[[float, complex], int] = cb_d17
    bad_d17: Callable[[int], float] = cb_c17

def fn18(
    cb_a18: Callable[[float], int],
    cb_b18: Callable[[float], float],
    cb_c18: Callable[[float], complex],
    cb_d18: Callable[[float, float], int],
) -> None:
    ok_a18: Callable[[int], float] = cb_a18
    ok_b18: Callable[[float], float] = cb_b18
    bad_a18: Callable[[complex], int] = cb_a18
    bad_b18: Callable[[complex], float] = cb_b18
    bad_c18: Callable[[complex, complex], int] = cb_d18
    bad_d18: Callable[[float, complex], int] = cb_d18

def fn19(
    cb_a19: Callable[[float], int],
    cb_b19: Callable[[float], float],
    cb_c19: Callable[[float], complex],
    cb_d19: Callable[[float, float], int],
) -> None:
    ok_a19: Callable[[float], complex] = cb_c19
    ok_b19: Callable[[int], complex] = cb_c19
    bad_a19: Callable[[float], float] = cb_c19
    bad_b19: Callable[[int], float] = cb_c19
    bad_c19: Callable[[complex, float], int] = cb_d19
    bad_d19: Callable[[complex], float] = cb_b19

def fn20(
    cb_a20: Callable[[float], int],
    cb_b20: Callable[[float], float],
    cb_c20: Callable[[float], complex],
    cb_d20: Callable[[float, float], int],
) -> None:
    ok_a20: Callable[[int], int] = cb_a20
    ok_b20: Callable[[int, float], float] = cb_d20
    bad_a20: Callable[[complex], int] = cb_a20
    bad_b20: Callable[[float], float] = cb_c20
    bad_c20: Callable[[float, complex], int] = cb_d20
    bad_d20: Callable[[int], float] = cb_c20

def fn21(
    cb_a21: Callable[[float], int],
    cb_b21: Callable[[float], float],
    cb_c21: Callable[[float], complex],
    cb_d21: Callable[[float, float], int],
) -> None:
    ok_a21: Callable[[int], float] = cb_a21
    ok_b21: Callable[[float], float] = cb_b21
    bad_a21: Callable[[complex], int] = cb_a21
    bad_b21: Callable[[complex], float] = cb_b21
    bad_c21: Callable[[complex, complex], int] = cb_d21
    bad_d21: Callable[[float, complex], int] = cb_d21

def fn22(
    cb_a22: Callable[[float], int],
    cb_b22: Callable[[float], float],
    cb_c22: Callable[[float], complex],
    cb_d22: Callable[[float, float], int],
) -> None:
    ok_a22: Callable[[float], complex] = cb_c22
    ok_b22: Callable[[int], complex] = cb_c22
    bad_a22: Callable[[float], float] = cb_c22
    bad_b22: Callable[[int], float] = cb_c22
    bad_c22: Callable[[complex, float], int] = cb_d22
    bad_d22: Callable[[complex], float] = cb_b22

def fn23(
    cb_a23: Callable[[float], int],
    cb_b23: Callable[[float], float],
    cb_c23: Callable[[float], complex],
    cb_d23: Callable[[float, float], int],
) -> None:
    ok_a23: Callable[[int], int] = cb_a23
    ok_b23: Callable[[int, float], float] = cb_d23
    bad_a23: Callable[[complex], int] = cb_a23
    bad_b23: Callable[[float], float] = cb_c23
    bad_c23: Callable[[float, complex], int] = cb_d23
    bad_d23: Callable[[int], float] = cb_c23

def fn24(
    cb_a24: Callable[[float], int],
    cb_b24: Callable[[float], float],
    cb_c24: Callable[[float], complex],
    cb_d24: Callable[[float, float], int],
) -> None:
    ok_a24: Callable[[int], float] = cb_a24
    ok_b24: Callable[[float], float] = cb_b24
    bad_a24: Callable[[complex], int] = cb_a24
    bad_b24: Callable[[complex], float] = cb_b24
    bad_c24: Callable[[complex, complex], int] = cb_d24
    bad_d24: Callable[[float, complex], int] = cb_d24

def fn25(
    cb_a25: Callable[[float], int],
    cb_b25: Callable[[float], float],
    cb_c25: Callable[[float], complex],
    cb_d25: Callable[[float, float], int],
) -> None:
    ok_a25: Callable[[float], complex] = cb_c25
    ok_b25: Callable[[int], complex] = cb_c25
    bad_a25: Callable[[float], float] = cb_c25
    bad_b25: Callable[[int], float] = cb_c25
    bad_c25: Callable[[complex, float], int] = cb_d25
    bad_d25: Callable[[complex], float] = cb_b25

def fn26(
    cb_a26: Callable[[float], int],
    cb_b26: Callable[[float], float],
    cb_c26: Callable[[float], complex],
    cb_d26: Callable[[float, float], int],
) -> None:
    ok_a26: Callable[[int], int] = cb_a26
    ok_b26: Callable[[int, float], float] = cb_d26
    bad_a26: Callable[[complex], int] = cb_a26
    bad_b26: Callable[[float], float] = cb_c26
    bad_c26: Callable[[float, complex], int] = cb_d26
    bad_d26: Callable[[int], float] = cb_c26

def fn27(
    cb_a27: Callable[[float], int],
    cb_b27: Callable[[float], float],
    cb_c27: Callable[[float], complex],
    cb_d27: Callable[[float, float], int],
) -> None:
    ok_a27: Callable[[int], float] = cb_a27
    ok_b27: Callable[[float], float] = cb_b27
    bad_a27: Callable[[complex], int] = cb_a27
    bad_b27: Callable[[complex], float] = cb_b27
    bad_c27: Callable[[complex, complex], int] = cb_d27
    bad_d27: Callable[[float, complex], int] = cb_d27

def fn28(
    cb_a28: Callable[[float], int],
    cb_b28: Callable[[float], float],
    cb_c28: Callable[[float], complex],
    cb_d28: Callable[[float, float], int],
) -> None:
    ok_a28: Callable[[float], complex] = cb_c28
    ok_b28: Callable[[int], complex] = cb_c28
    bad_a28: Callable[[float], float] = cb_c28
    bad_b28: Callable[[int], float] = cb_c28
    bad_c28: Callable[[complex, float], int] = cb_d28
    bad_d28: Callable[[complex], float] = cb_b28

def fn29(
    cb_a29: Callable[[float], int],
    cb_b29: Callable[[float], float],
    cb_c29: Callable[[float], complex],
    cb_d29: Callable[[float, float], int],
) -> None:
    ok_a29: Callable[[int], int] = cb_a29
    ok_b29: Callable[[int, float], float] = cb_d29
    bad_a29: Callable[[complex], int] = cb_a29
    bad_b29: Callable[[float], float] = cb_c29
    bad_c29: Callable[[float, complex], int] = cb_d29
    bad_d29: Callable[[int], float] = cb_c29

def fn30(
    cb_a30: Callable[[float], int],
    cb_b30: Callable[[float], float],
    cb_c30: Callable[[float], complex],
    cb_d30: Callable[[float, float], int],
) -> None:
    ok_a30: Callable[[int], float] = cb_a30
    ok_b30: Callable[[float], float] = cb_b30
    bad_a30: Callable[[complex], int] = cb_a30
    bad_b30: Callable[[complex], float] = cb_b30
    bad_c30: Callable[[complex, complex], int] = cb_d30
    bad_d30: Callable[[float, complex], int] = cb_d30

def fn31(
    cb_a31: Callable[[float], int],
    cb_b31: Callable[[float], float],
    cb_c31: Callable[[float], complex],
    cb_d31: Callable[[float, float], int],
) -> None:
    ok_a31: Callable[[float], complex] = cb_c31
    ok_b31: Callable[[int], complex] = cb_c31
    bad_a31: Callable[[float], float] = cb_c31
    bad_b31: Callable[[int], float] = cb_c31
    bad_c31: Callable[[complex, float], int] = cb_d31
    bad_d31: Callable[[complex], float] = cb_b31

def fn32(
    cb_a32: Callable[[float], int],
    cb_b32: Callable[[float], float],
    cb_c32: Callable[[float], complex],
    cb_d32: Callable[[float, float], int],
) -> None:
    ok_a32: Callable[[int], int] = cb_a32
    ok_b32: Callable[[int, float], float] = cb_d32
    bad_a32: Callable[[complex], int] = cb_a32
    bad_b32: Callable[[float], float] = cb_c32
    bad_c32: Callable[[float, complex], int] = cb_d32
    bad_d32: Callable[[int], float] = cb_c32

def fn33(
    cb_a33: Callable[[float], int],
    cb_b33: Callable[[float], float],
    cb_c33: Callable[[float], complex],
    cb_d33: Callable[[float, float], int],
) -> None:
    ok_a33: Callable[[int], float] = cb_a33
    ok_b33: Callable[[float], float] = cb_b33
    bad_a33: Callable[[complex], int] = cb_a33
    bad_b33: Callable[[complex], float] = cb_b33
    bad_c33: Callable[[complex, complex], int] = cb_d33
    bad_d33: Callable[[float, complex], int] = cb_d33

def fn34(
    cb_a34: Callable[[float], int],
    cb_b34: Callable[[float], float],
    cb_c34: Callable[[float], complex],
    cb_d34: Callable[[float, float], int],
) -> None:
    ok_a34: Callable[[float], complex] = cb_c34
    ok_b34: Callable[[int], complex] = cb_c34
    bad_a34: Callable[[float], float] = cb_c34
    bad_b34: Callable[[int], float] = cb_c34
    bad_c34: Callable[[complex, float], int] = cb_d34
    bad_d34: Callable[[complex], float] = cb_b34

def fn35(
    cb_a35: Callable[[float], int],
    cb_b35: Callable[[float], float],
    cb_c35: Callable[[float], complex],
    cb_d35: Callable[[float, float], int],
) -> None:
    ok_a35: Callable[[int], int] = cb_a35
    ok_b35: Callable[[int, float], float] = cb_d35
    bad_a35: Callable[[complex], int] = cb_a35
    bad_b35: Callable[[float], float] = cb_c35
    bad_c35: Callable[[float, complex], int] = cb_d35
    bad_d35: Callable[[int], float] = cb_c35

def fn36(
    cb_a36: Callable[[float], int],
    cb_b36: Callable[[float], float],
    cb_c36: Callable[[float], complex],
    cb_d36: Callable[[float, float], int],
) -> None:
    ok_a36: Callable[[int], float] = cb_a36
    ok_b36: Callable[[float], float] = cb_b36
    bad_a36: Callable[[complex], int] = cb_a36
    bad_b36: Callable[[complex], float] = cb_b36
    bad_c36: Callable[[complex, complex], int] = cb_d36
    bad_d36: Callable[[float, complex], int] = cb_d36

def fn37(
    cb_a37: Callable[[float], int],
    cb_b37: Callable[[float], float],
    cb_c37: Callable[[float], complex],
    cb_d37: Callable[[float, float], int],
) -> None:
    ok_a37: Callable[[float], complex] = cb_c37
    ok_b37: Callable[[int], complex] = cb_c37
    bad_a37: Callable[[float], float] = cb_c37
    bad_b37: Callable[[int], float] = cb_c37
    bad_c37: Callable[[complex, float], int] = cb_d37
    bad_d37: Callable[[complex], float] = cb_b37

def fn38(
    cb_a38: Callable[[float], int],
    cb_b38: Callable[[float], float],
    cb_c38: Callable[[float], complex],
    cb_d38: Callable[[float, float], int],
) -> None:
    ok_a38: Callable[[int], int] = cb_a38
    ok_b38: Callable[[int, float], float] = cb_d38
    bad_a38: Callable[[complex], int] = cb_a38
    bad_b38: Callable[[float], float] = cb_c38
    bad_c38: Callable[[float, complex], int] = cb_d38
    bad_d38: Callable[[int], float] = cb_c38

def fn39(
    cb_a39: Callable[[float], int],
    cb_b39: Callable[[float], float],
    cb_c39: Callable[[float], complex],
    cb_d39: Callable[[float, float], int],
) -> None:
    ok_a39: Callable[[int], float] = cb_a39
    ok_b39: Callable[[float], float] = cb_b39
    bad_a39: Callable[[complex], int] = cb_a39
    bad_b39: Callable[[complex], float] = cb_b39
    bad_c39: Callable[[complex, complex], int] = cb_d39
    bad_d39: Callable[[float, complex], int] = cb_d39

def fn40(
    cb_a40: Callable[[float], int],
    cb_b40: Callable[[float], float],
    cb_c40: Callable[[float], complex],
    cb_d40: Callable[[float, float], int],
) -> None:
    ok_a40: Callable[[float], complex] = cb_c40
    ok_b40: Callable[[int], complex] = cb_c40
    bad_a40: Callable[[float], float] = cb_c40
    bad_b40: Callable[[int], float] = cb_c40
    bad_c40: Callable[[complex, float], int] = cb_d40
    bad_d40: Callable[[complex], float] = cb_b40

def fn41(
    cb_a41: Callable[[float], int],
    cb_b41: Callable[[float], float],
    cb_c41: Callable[[float], complex],
    cb_d41: Callable[[float, float], int],
) -> None:
    ok_a41: Callable[[int], int] = cb_a41
    ok_b41: Callable[[int, float], float] = cb_d41
    bad_a41: Callable[[complex], int] = cb_a41
    bad_b41: Callable[[float], float] = cb_c41
    bad_c41: Callable[[float, complex], int] = cb_d41
    bad_d41: Callable[[int], float] = cb_c41

def fn42(
    cb_a42: Callable[[float], int],
    cb_b42: Callable[[float], float],
    cb_c42: Callable[[float], complex],
    cb_d42: Callable[[float, float], int],
) -> None:
    ok_a42: Callable[[int], float] = cb_a42
    ok_b42: Callable[[float], float] = cb_b42
    bad_a42: Callable[[complex], int] = cb_a42
    bad_b42: Callable[[complex], float] = cb_b42
    bad_c42: Callable[[complex, complex], int] = cb_d42
    bad_d42: Callable[[float, complex], int] = cb_d42

def fn43(
    cb_a43: Callable[[float], int],
    cb_b43: Callable[[float], float],
    cb_c43: Callable[[float], complex],
    cb_d43: Callable[[float, float], int],
) -> None:
    ok_a43: Callable[[float], complex] = cb_c43
    ok_b43: Callable[[int], complex] = cb_c43
    bad_a43: Callable[[float], float] = cb_c43
    bad_b43: Callable[[int], float] = cb_c43
    bad_c43: Callable[[complex, float], int] = cb_d43
    bad_d43: Callable[[complex], float] = cb_b43

def fn44(
    cb_a44: Callable[[float], int],
    cb_b44: Callable[[float], float],
    cb_c44: Callable[[float], complex],
    cb_d44: Callable[[float, float], int],
) -> None:
    ok_a44: Callable[[int], int] = cb_a44
    ok_b44: Callable[[int, float], float] = cb_d44
    bad_a44: Callable[[complex], int] = cb_a44
    bad_b44: Callable[[float], float] = cb_c44
    bad_c44: Callable[[float, complex], int] = cb_d44
    bad_d44: Callable[[int], float] = cb_c44

def fn45(
    cb_a45: Callable[[float], int],
    cb_b45: Callable[[float], float],
    cb_c45: Callable[[float], complex],
    cb_d45: Callable[[float, float], int],
) -> None:
    ok_a45: Callable[[int], float] = cb_a45
    ok_b45: Callable[[float], float] = cb_b45
    bad_a45: Callable[[complex], int] = cb_a45
    bad_b45: Callable[[complex], float] = cb_b45
    bad_c45: Callable[[complex, complex], int] = cb_d45
    bad_d45: Callable[[float, complex], int] = cb_d45

def fn46(
    cb_a46: Callable[[float], int],
    cb_b46: Callable[[float], float],
    cb_c46: Callable[[float], complex],
    cb_d46: Callable[[float, float], int],
) -> None:
    ok_a46: Callable[[float], complex] = cb_c46
    ok_b46: Callable[[int], complex] = cb_c46
    bad_a46: Callable[[float], float] = cb_c46
    bad_b46: Callable[[int], float] = cb_c46
    bad_c46: Callable[[complex, float], int] = cb_d46
    bad_d46: Callable[[complex], float] = cb_b46

def fn47(
    cb_a47: Callable[[float], int],
    cb_b47: Callable[[float], float],
    cb_c47: Callable[[float], complex],
    cb_d47: Callable[[float, float], int],
) -> None:
    ok_a47: Callable[[int], int] = cb_a47
    ok_b47: Callable[[int, float], float] = cb_d47
    bad_a47: Callable[[complex], int] = cb_a47
    bad_b47: Callable[[float], float] = cb_c47
    bad_c47: Callable[[float, complex], int] = cb_d47
    bad_d47: Callable[[int], float] = cb_c47

def fn48(
    cb_a48: Callable[[float], int],
    cb_b48: Callable[[float], float],
    cb_c48: Callable[[float], complex],
    cb_d48: Callable[[float, float], int],
) -> None:
    ok_a48: Callable[[int], float] = cb_a48
    ok_b48: Callable[[float], float] = cb_b48
    bad_a48: Callable[[complex], int] = cb_a48
    bad_b48: Callable[[complex], float] = cb_b48
    bad_c48: Callable[[complex, complex], int] = cb_d48
    bad_d48: Callable[[float, complex], int] = cb_d48

def fn49(
    cb_a49: Callable[[float], int],
    cb_b49: Callable[[float], float],
    cb_c49: Callable[[float], complex],
    cb_d49: Callable[[float, float], int],
) -> None:
    ok_a49: Callable[[float], complex] = cb_c49
    ok_b49: Callable[[int], complex] = cb_c49
    bad_a49: Callable[[float], float] = cb_c49
    bad_b49: Callable[[int], float] = cb_c49
    bad_c49: Callable[[complex, float], int] = cb_d49
    bad_d49: Callable[[complex], float] = cb_b49

def fn50(
    cb_a50: Callable[[float], int],
    cb_b50: Callable[[float], float],
    cb_c50: Callable[[float], complex],
    cb_d50: Callable[[float, float], int],
) -> None:
    ok_a50: Callable[[int], int] = cb_a50
    ok_b50: Callable[[int, float], float] = cb_d50
    bad_a50: Callable[[complex], int] = cb_a50
    bad_b50: Callable[[float], float] = cb_c50
    bad_c50: Callable[[float, complex], int] = cb_d50
    bad_d50: Callable[[int], float] = cb_c50

def fn51(
    cb_a51: Callable[[float], int],
    cb_b51: Callable[[float], float],
    cb_c51: Callable[[float], complex],
    cb_d51: Callable[[float, float], int],
) -> None:
    ok_a51: Callable[[int], float] = cb_a51
    ok_b51: Callable[[float], float] = cb_b51
    bad_a51: Callable[[complex], int] = cb_a51
    bad_b51: Callable[[complex], float] = cb_b51
    bad_c51: Callable[[complex, complex], int] = cb_d51
    bad_d51: Callable[[float, complex], int] = cb_d51

def fn52(
    cb_a52: Callable[[float], int],
    cb_b52: Callable[[float], float],
    cb_c52: Callable[[float], complex],
    cb_d52: Callable[[float, float], int],
) -> None:
    ok_a52: Callable[[float], complex] = cb_c52
    ok_b52: Callable[[int], complex] = cb_c52
    bad_a52: Callable[[float], float] = cb_c52
    bad_b52: Callable[[int], float] = cb_c52
    bad_c52: Callable[[complex, float], int] = cb_d52
    bad_d52: Callable[[complex], float] = cb_b52

def fn53(
    cb_a53: Callable[[float], int],
    cb_b53: Callable[[float], float],
    cb_c53: Callable[[float], complex],
    cb_d53: Callable[[float, float], int],
) -> None:
    ok_a53: Callable[[int], int] = cb_a53
    ok_b53: Callable[[int, float], float] = cb_d53
    bad_a53: Callable[[complex], int] = cb_a53
    bad_b53: Callable[[float], float] = cb_c53
    bad_c53: Callable[[float, complex], int] = cb_d53
    bad_d53: Callable[[int], float] = cb_c53

def fn54(
    cb_a54: Callable[[float], int],
    cb_b54: Callable[[float], float],
    cb_c54: Callable[[float], complex],
    cb_d54: Callable[[float, float], int],
) -> None:
    ok_a54: Callable[[int], float] = cb_a54
    ok_b54: Callable[[float], float] = cb_b54
    bad_a54: Callable[[complex], int] = cb_a54
    bad_b54: Callable[[complex], float] = cb_b54
    bad_c54: Callable[[complex, complex], int] = cb_d54
    bad_d54: Callable[[float, complex], int] = cb_d54

def fn55(
    cb_a55: Callable[[float], int],
    cb_b55: Callable[[float], float],
    cb_c55: Callable[[float], complex],
    cb_d55: Callable[[float, float], int],
) -> None:
    ok_a55: Callable[[float], complex] = cb_c55
    ok_b55: Callable[[int], complex] = cb_c55
    bad_a55: Callable[[float], float] = cb_c55
    bad_b55: Callable[[int], float] = cb_c55
    bad_c55: Callable[[complex, float], int] = cb_d55
    bad_d55: Callable[[complex], float] = cb_b55

def fn56(
    cb_a56: Callable[[float], int],
    cb_b56: Callable[[float], float],
    cb_c56: Callable[[float], complex],
    cb_d56: Callable[[float, float], int],
) -> None:
    ok_a56: Callable[[int], int] = cb_a56
    ok_b56: Callable[[int, float], float] = cb_d56
    bad_a56: Callable[[complex], int] = cb_a56
    bad_b56: Callable[[float], float] = cb_c56
    bad_c56: Callable[[float, complex], int] = cb_d56
    bad_d56: Callable[[int], float] = cb_c56

def fn57(
    cb_a57: Callable[[float], int],
    cb_b57: Callable[[float], float],
    cb_c57: Callable[[float], complex],
    cb_d57: Callable[[float, float], int],
) -> None:
    ok_a57: Callable[[int], float] = cb_a57
    ok_b57: Callable[[float], float] = cb_b57
    bad_a57: Callable[[complex], int] = cb_a57
    bad_b57: Callable[[complex], float] = cb_b57
    bad_c57: Callable[[complex, complex], int] = cb_d57
    bad_d57: Callable[[float, complex], int] = cb_d57

def fn58(
    cb_a58: Callable[[float], int],
    cb_b58: Callable[[float], float],
    cb_c58: Callable[[float], complex],
    cb_d58: Callable[[float, float], int],
) -> None:
    ok_a58: Callable[[float], complex] = cb_c58
    ok_b58: Callable[[int], complex] = cb_c58
    bad_a58: Callable[[float], float] = cb_c58
    bad_b58: Callable[[int], float] = cb_c58
    bad_c58: Callable[[complex, float], int] = cb_d58
    bad_d58: Callable[[complex], float] = cb_b58

def fn59(
    cb_a59: Callable[[float], int],
    cb_b59: Callable[[float], float],
    cb_c59: Callable[[float], complex],
    cb_d59: Callable[[float, float], int],
) -> None:
    ok_a59: Callable[[int], int] = cb_a59
    ok_b59: Callable[[int, float], float] = cb_d59
    bad_a59: Callable[[complex], int] = cb_a59
    bad_b59: Callable[[float], float] = cb_c59
    bad_c59: Callable[[float, complex], int] = cb_d59
    bad_d59: Callable[[int], float] = cb_c59

def fn60(
    cb_a60: Callable[[float], int],
    cb_b60: Callable[[float], float],
    cb_c60: Callable[[float], complex],
    cb_d60: Callable[[float, float], int],
) -> None:
    ok_a60: Callable[[int], float] = cb_a60
    ok_b60: Callable[[float], float] = cb_b60
    bad_a60: Callable[[complex], int] = cb_a60
    bad_b60: Callable[[complex], float] = cb_b60
    bad_c60: Callable[[complex, complex], int] = cb_d60
    bad_d60: Callable[[float, complex], int] = cb_d60

def fn61(
    cb_a61: Callable[[float], int],
    cb_b61: Callable[[float], float],
    cb_c61: Callable[[float], complex],
    cb_d61: Callable[[float, float], int],
) -> None:
    ok_a61: Callable[[float], complex] = cb_c61
    ok_b61: Callable[[int], complex] = cb_c61
    bad_a61: Callable[[float], float] = cb_c61
    bad_b61: Callable[[int], float] = cb_c61
    bad_c61: Callable[[complex, float], int] = cb_d61
    bad_d61: Callable[[complex], float] = cb_b61

def fn62(
    cb_a62: Callable[[float], int],
    cb_b62: Callable[[float], float],
    cb_c62: Callable[[float], complex],
    cb_d62: Callable[[float, float], int],
) -> None:
    ok_a62: Callable[[int], int] = cb_a62
    ok_b62: Callable[[int, float], float] = cb_d62
    bad_a62: Callable[[complex], int] = cb_a62
    bad_b62: Callable[[float], float] = cb_c62
    bad_c62: Callable[[float, complex], int] = cb_d62
    bad_d62: Callable[[int], float] = cb_c62

def fn63(
    cb_a63: Callable[[float], int],
    cb_b63: Callable[[float], float],
    cb_c63: Callable[[float], complex],
    cb_d63: Callable[[float, float], int],
) -> None:
    ok_a63: Callable[[int], float] = cb_a63
    ok_b63: Callable[[float], float] = cb_b63
    bad_a63: Callable[[complex], int] = cb_a63
    bad_b63: Callable[[complex], float] = cb_b63
    bad_c63: Callable[[complex, complex], int] = cb_d63
    bad_d63: Callable[[float, complex], int] = cb_d63

def fn64(
    cb_a64: Callable[[float], int],
    cb_b64: Callable[[float], float],
    cb_c64: Callable[[float], complex],
    cb_d64: Callable[[float, float], int],
) -> None:
    ok_a64: Callable[[float], complex] = cb_c64
    ok_b64: Callable[[int], complex] = cb_c64
    bad_a64: Callable[[float], float] = cb_c64
    bad_b64: Callable[[int], float] = cb_c64
    bad_c64: Callable[[complex, float], int] = cb_d64
    bad_d64: Callable[[complex], float] = cb_b64

def fn65(
    cb_a65: Callable[[float], int],
    cb_b65: Callable[[float], float],
    cb_c65: Callable[[float], complex],
    cb_d65: Callable[[float, float], int],
) -> None:
    ok_a65: Callable[[int], int] = cb_a65
    ok_b65: Callable[[int, float], float] = cb_d65
    bad_a65: Callable[[complex], int] = cb_a65
    bad_b65: Callable[[float], float] = cb_c65
    bad_c65: Callable[[float, complex], int] = cb_d65
    bad_d65: Callable[[int], float] = cb_c65

def fn66(
    cb_a66: Callable[[float], int],
    cb_b66: Callable[[float], float],
    cb_c66: Callable[[float], complex],
    cb_d66: Callable[[float, float], int],
) -> None:
    ok_a66: Callable[[int], float] = cb_a66
    ok_b66: Callable[[float], float] = cb_b66
    bad_a66: Callable[[complex], int] = cb_a66
    bad_b66: Callable[[complex], float] = cb_b66
    bad_c66: Callable[[complex, complex], int] = cb_d66
    bad_d66: Callable[[float, complex], int] = cb_d66

def fn67(
    cb_a67: Callable[[float], int],
    cb_b67: Callable[[float], float],
    cb_c67: Callable[[float], complex],
    cb_d67: Callable[[float, float], int],
) -> None:
    ok_a67: Callable[[float], complex] = cb_c67
    ok_b67: Callable[[int], complex] = cb_c67
    bad_a67: Callable[[float], float] = cb_c67
    bad_b67: Callable[[int], float] = cb_c67
    bad_c67: Callable[[complex, float], int] = cb_d67
    bad_d67: Callable[[complex], float] = cb_b67

def fn68(
    cb_a68: Callable[[float], int],
    cb_b68: Callable[[float], float],
    cb_c68: Callable[[float], complex],
    cb_d68: Callable[[float, float], int],
) -> None:
    ok_a68: Callable[[int], int] = cb_a68
    ok_b68: Callable[[int, float], float] = cb_d68
    bad_a68: Callable[[complex], int] = cb_a68
    bad_b68: Callable[[float], float] = cb_c68
    bad_c68: Callable[[float, complex], int] = cb_d68
    bad_d68: Callable[[int], float] = cb_c68

def fn69(
    cb_a69: Callable[[float], int],
    cb_b69: Callable[[float], float],
    cb_c69: Callable[[float], complex],
    cb_d69: Callable[[float, float], int],
) -> None:
    ok_a69: Callable[[int], float] = cb_a69
    ok_b69: Callable[[float], float] = cb_b69
    bad_a69: Callable[[complex], int] = cb_a69
    bad_b69: Callable[[complex], float] = cb_b69
    bad_c69: Callable[[complex, complex], int] = cb_d69
    bad_d69: Callable[[float, complex], int] = cb_d69

def fn70(
    cb_a70: Callable[[float], int],
    cb_b70: Callable[[float], float],
    cb_c70: Callable[[float], complex],
    cb_d70: Callable[[float, float], int],
) -> None:
    ok_a70: Callable[[float], complex] = cb_c70
    ok_b70: Callable[[int], complex] = cb_c70
    bad_a70: Callable[[float], float] = cb_c70
    bad_b70: Callable[[int], float] = cb_c70
    bad_c70: Callable[[complex, float], int] = cb_d70
    bad_d70: Callable[[complex], float] = cb_b70

def fn71(
    cb_a71: Callable[[float], int],
    cb_b71: Callable[[float], float],
    cb_c71: Callable[[float], complex],
    cb_d71: Callable[[float, float], int],
) -> None:
    ok_a71: Callable[[int], int] = cb_a71
    ok_b71: Callable[[int, float], float] = cb_d71
    bad_a71: Callable[[complex], int] = cb_a71
    bad_b71: Callable[[float], float] = cb_c71
    bad_c71: Callable[[float, complex], int] = cb_d71
    bad_d71: Callable[[int], float] = cb_c71

def fn72(
    cb_a72: Callable[[float], int],
    cb_b72: Callable[[float], float],
    cb_c72: Callable[[float], complex],
    cb_d72: Callable[[float, float], int],
) -> None:
    ok_a72: Callable[[int], float] = cb_a72
    ok_b72: Callable[[float], float] = cb_b72
    bad_a72: Callable[[complex], int] = cb_a72
    bad_b72: Callable[[complex], float] = cb_b72
    bad_c72: Callable[[complex, complex], int] = cb_d72
    bad_d72: Callable[[float, complex], int] = cb_d72

def fn73(
    cb_a73: Callable[[float], int],
    cb_b73: Callable[[float], float],
    cb_c73: Callable[[float], complex],
    cb_d73: Callable[[float, float], int],
) -> None:
    ok_a73: Callable[[float], complex] = cb_c73
    ok_b73: Callable[[int], complex] = cb_c73
    bad_a73: Callable[[float], float] = cb_c73
    bad_b73: Callable[[int], float] = cb_c73
    bad_c73: Callable[[complex, float], int] = cb_d73
    bad_d73: Callable[[complex], float] = cb_b73

def fn74(
    cb_a74: Callable[[float], int],
    cb_b74: Callable[[float], float],
    cb_c74: Callable[[float], complex],
    cb_d74: Callable[[float, float], int],
) -> None:
    ok_a74: Callable[[int], int] = cb_a74
    ok_b74: Callable[[int, float], float] = cb_d74
    bad_a74: Callable[[complex], int] = cb_a74
    bad_b74: Callable[[float], float] = cb_c74
    bad_c74: Callable[[float, complex], int] = cb_d74
    bad_d74: Callable[[int], float] = cb_c74

def fn75(
    cb_a75: Callable[[float], int],
    cb_b75: Callable[[float], float],
    cb_c75: Callable[[float], complex],
    cb_d75: Callable[[float, float], int],
) -> None:
    ok_a75: Callable[[int], float] = cb_a75
    ok_b75: Callable[[float], float] = cb_b75
    bad_a75: Callable[[complex], int] = cb_a75
    bad_b75: Callable[[complex], float] = cb_b75
    bad_c75: Callable[[complex, complex], int] = cb_d75
    bad_d75: Callable[[float, complex], int] = cb_d75

def fn76(
    cb_a76: Callable[[float], int],
    cb_b76: Callable[[float], float],
    cb_c76: Callable[[float], complex],
    cb_d76: Callable[[float, float], int],
) -> None:
    ok_a76: Callable[[float], complex] = cb_c76
    ok_b76: Callable[[int], complex] = cb_c76
    bad_a76: Callable[[float], float] = cb_c76
    bad_b76: Callable[[int], float] = cb_c76
    bad_c76: Callable[[complex, float], int] = cb_d76
    bad_d76: Callable[[complex], float] = cb_b76

def fn77(
    cb_a77: Callable[[float], int],
    cb_b77: Callable[[float], float],
    cb_c77: Callable[[float], complex],
    cb_d77: Callable[[float, float], int],
) -> None:
    ok_a77: Callable[[int], int] = cb_a77
    ok_b77: Callable[[int, float], float] = cb_d77
    bad_a77: Callable[[complex], int] = cb_a77
    bad_b77: Callable[[float], float] = cb_c77
    bad_c77: Callable[[float, complex], int] = cb_d77
    bad_d77: Callable[[int], float] = cb_c77

def fn78(
    cb_a78: Callable[[float], int],
    cb_b78: Callable[[float], float],
    cb_c78: Callable[[float], complex],
    cb_d78: Callable[[float, float], int],
) -> None:
    ok_a78: Callable[[int], float] = cb_a78
    ok_b78: Callable[[float], float] = cb_b78
    bad_a78: Callable[[complex], int] = cb_a78
    bad_b78: Callable[[complex], float] = cb_b78
    bad_c78: Callable[[complex, complex], int] = cb_d78
    bad_d78: Callable[[float, complex], int] = cb_d78

def fn79(
    cb_a79: Callable[[float], int],
    cb_b79: Callable[[float], float],
    cb_c79: Callable[[float], complex],
    cb_d79: Callable[[float, float], int],
) -> None:
    ok_a79: Callable[[float], complex] = cb_c79
    ok_b79: Callable[[int], complex] = cb_c79
    bad_a79: Callable[[float], float] = cb_c79
    bad_b79: Callable[[int], float] = cb_c79
    bad_c79: Callable[[complex, float], int] = cb_d79
    bad_d79: Callable[[complex], float] = cb_b79

def fn80(
    cb_a80: Callable[[float], int],
    cb_b80: Callable[[float], float],
    cb_c80: Callable[[float], complex],
    cb_d80: Callable[[float, float], int],
) -> None:
    ok_a80: Callable[[int], int] = cb_a80
    ok_b80: Callable[[int, float], float] = cb_d80
    bad_a80: Callable[[complex], int] = cb_a80
    bad_b80: Callable[[float], float] = cb_c80
    bad_c80: Callable[[float, complex], int] = cb_d80
    bad_d80: Callable[[int], float] = cb_c80

def fn81(
    cb_a81: Callable[[float], int],
    cb_b81: Callable[[float], float],
    cb_c81: Callable[[float], complex],
    cb_d81: Callable[[float, float], int],
) -> None:
    ok_a81: Callable[[int], float] = cb_a81
    ok_b81: Callable[[float], float] = cb_b81
    bad_a81: Callable[[complex], int] = cb_a81
    bad_b81: Callable[[complex], float] = cb_b81
    bad_c81: Callable[[complex, complex], int] = cb_d81
    bad_d81: Callable[[float, complex], int] = cb_d81

def fn82(
    cb_a82: Callable[[float], int],
    cb_b82: Callable[[float], float],
    cb_c82: Callable[[float], complex],
    cb_d82: Callable[[float, float], int],
) -> None:
    ok_a82: Callable[[float], complex] = cb_c82
    ok_b82: Callable[[int], complex] = cb_c82
    bad_a82: Callable[[float], float] = cb_c82
    bad_b82: Callable[[int], float] = cb_c82
    bad_c82: Callable[[complex, float], int] = cb_d82
    bad_d82: Callable[[complex], float] = cb_b82

def fn83(
    cb_a83: Callable[[float], int],
    cb_b83: Callable[[float], float],
    cb_c83: Callable[[float], complex],
    cb_d83: Callable[[float, float], int],
) -> None:
    ok_a83: Callable[[int], int] = cb_a83
    ok_b83: Callable[[int, float], float] = cb_d83
    bad_a83: Callable[[complex], int] = cb_a83
    bad_b83: Callable[[float], float] = cb_c83
    bad_c83: Callable[[float, complex], int] = cb_d83
    bad_d83: Callable[[int], float] = cb_c83

def fn84(
    cb_a84: Callable[[float], int],
    cb_b84: Callable[[float], float],
    cb_c84: Callable[[float], complex],
    cb_d84: Callable[[float, float], int],
) -> None:
    ok_a84: Callable[[int], float] = cb_a84
    ok_b84: Callable[[float], float] = cb_b84
    bad_a84: Callable[[complex], int] = cb_a84
    bad_b84: Callable[[complex], float] = cb_b84
    bad_c84: Callable[[complex, complex], int] = cb_d84
    bad_d84: Callable[[float, complex], int] = cb_d84

def fn85(
    cb_a85: Callable[[float], int],
    cb_b85: Callable[[float], float],
    cb_c85: Callable[[float], complex],
    cb_d85: Callable[[float, float], int],
) -> None:
    ok_a85: Callable[[float], complex] = cb_c85
    ok_b85: Callable[[int], complex] = cb_c85
    bad_a85: Callable[[float], float] = cb_c85
    bad_b85: Callable[[int], float] = cb_c85
    bad_c85: Callable[[complex, float], int] = cb_d85
    bad_d85: Callable[[complex], float] = cb_b85

def fn86(
    cb_a86: Callable[[float], int],
    cb_b86: Callable[[float], float],
    cb_c86: Callable[[float], complex],
    cb_d86: Callable[[float, float], int],
) -> None:
    ok_a86: Callable[[int], int] = cb_a86
    ok_b86: Callable[[int, float], float] = cb_d86
    bad_a86: Callable[[complex], int] = cb_a86
    bad_b86: Callable[[float], float] = cb_c86
    bad_c86: Callable[[float, complex], int] = cb_d86
    bad_d86: Callable[[int], float] = cb_c86

def fn87(
    cb_a87: Callable[[float], int],
    cb_b87: Callable[[float], float],
    cb_c87: Callable[[float], complex],
    cb_d87: Callable[[float, float], int],
) -> None:
    ok_a87: Callable[[int], float] = cb_a87
    ok_b87: Callable[[float], float] = cb_b87
    bad_a87: Callable[[complex], int] = cb_a87
    bad_b87: Callable[[complex], float] = cb_b87
    bad_c87: Callable[[complex, complex], int] = cb_d87
    bad_d87: Callable[[float, complex], int] = cb_d87

def fn88(
    cb_a88: Callable[[float], int],
    cb_b88: Callable[[float], float],
    cb_c88: Callable[[float], complex],
    cb_d88: Callable[[float, float], int],
) -> None:
    ok_a88: Callable[[float], complex] = cb_c88
    ok_b88: Callable[[int], complex] = cb_c88
    bad_a88: Callable[[float], float] = cb_c88
    bad_b88: Callable[[int], float] = cb_c88
    bad_c88: Callable[[complex, float], int] = cb_d88
    bad_d88: Callable[[complex], float] = cb_b88

def fn89(
    cb_a89: Callable[[float], int],
    cb_b89: Callable[[float], float],
    cb_c89: Callable[[float], complex],
    cb_d89: Callable[[float, float], int],
) -> None:
    ok_a89: Callable[[int], int] = cb_a89
    ok_b89: Callable[[int, float], float] = cb_d89
    bad_a89: Callable[[complex], int] = cb_a89
    bad_b89: Callable[[float], float] = cb_c89
    bad_c89: Callable[[float, complex], int] = cb_d89
    bad_d89: Callable[[int], float] = cb_c89

def fn90(
    cb_a90: Callable[[float], int],
    cb_b90: Callable[[float], float],
    cb_c90: Callable[[float], complex],
    cb_d90: Callable[[float, float], int],
) -> None:
    ok_a90: Callable[[int], float] = cb_a90
    ok_b90: Callable[[float], float] = cb_b90
    bad_a90: Callable[[complex], int] = cb_a90
    bad_b90: Callable[[complex], float] = cb_b90
    bad_c90: Callable[[complex, complex], int] = cb_d90
    bad_d90: Callable[[float, complex], int] = cb_d90

def fn91(
    cb_a91: Callable[[float], int],
    cb_b91: Callable[[float], float],
    cb_c91: Callable[[float], complex],
    cb_d91: Callable[[float, float], int],
) -> None:
    ok_a91: Callable[[float], complex] = cb_c91
    ok_b91: Callable[[int], complex] = cb_c91
    bad_a91: Callable[[float], float] = cb_c91
    bad_b91: Callable[[int], float] = cb_c91
    bad_c91: Callable[[complex, float], int] = cb_d91
    bad_d91: Callable[[complex], float] = cb_b91

def fn92(
    cb_a92: Callable[[float], int],
    cb_b92: Callable[[float], float],
    cb_c92: Callable[[float], complex],
    cb_d92: Callable[[float, float], int],
) -> None:
    ok_a92: Callable[[int], int] = cb_a92
    ok_b92: Callable[[int, float], float] = cb_d92
    bad_a92: Callable[[complex], int] = cb_a92
    bad_b92: Callable[[float], float] = cb_c92
    bad_c92: Callable[[float, complex], int] = cb_d92
    bad_d92: Callable[[int], float] = cb_c92

def fn93(
    cb_a93: Callable[[float], int],
    cb_b93: Callable[[float], float],
    cb_c93: Callable[[float], complex],
    cb_d93: Callable[[float, float], int],
) -> None:
    ok_a93: Callable[[int], float] = cb_a93
    ok_b93: Callable[[float], float] = cb_b93
    bad_a93: Callable[[complex], int] = cb_a93
    bad_b93: Callable[[complex], float] = cb_b93
    bad_c93: Callable[[complex, complex], int] = cb_d93
    bad_d93: Callable[[float, complex], int] = cb_d93

def fn94(
    cb_a94: Callable[[float], int],
    cb_b94: Callable[[float], float],
    cb_c94: Callable[[float], complex],
    cb_d94: Callable[[float, float], int],
) -> None:
    ok_a94: Callable[[float], complex] = cb_c94
    ok_b94: Callable[[int], complex] = cb_c94
    bad_a94: Callable[[float], float] = cb_c94
    bad_b94: Callable[[int], float] = cb_c94
    bad_c94: Callable[[complex, float], int] = cb_d94
    bad_d94: Callable[[complex], float] = cb_b94

def fn95(
    cb_a95: Callable[[float], int],
    cb_b95: Callable[[float], float],
    cb_c95: Callable[[float], complex],
    cb_d95: Callable[[float, float], int],
) -> None:
    ok_a95: Callable[[int], int] = cb_a95
    ok_b95: Callable[[int, float], float] = cb_d95
    bad_a95: Callable[[complex], int] = cb_a95
    bad_b95: Callable[[float], float] = cb_c95
    bad_c95: Callable[[float, complex], int] = cb_d95
    bad_d95: Callable[[int], float] = cb_c95

def fn96(
    cb_a96: Callable[[float], int],
    cb_b96: Callable[[float], float],
    cb_c96: Callable[[float], complex],
    cb_d96: Callable[[float, float], int],
) -> None:
    ok_a96: Callable[[int], float] = cb_a96
    ok_b96: Callable[[float], float] = cb_b96
    bad_a96: Callable[[complex], int] = cb_a96
    bad_b96: Callable[[complex], float] = cb_b96
    bad_c96: Callable[[complex, complex], int] = cb_d96
    bad_d96: Callable[[float, complex], int] = cb_d96

def fn97(
    cb_a97: Callable[[float], int],
    cb_b97: Callable[[float], float],
    cb_c97: Callable[[float], complex],
    cb_d97: Callable[[float, float], int],
) -> None:
    ok_a97: Callable[[float], complex] = cb_c97
    ok_b97: Callable[[int], complex] = cb_c97
    bad_a97: Callable[[float], float] = cb_c97
    bad_b97: Callable[[int], float] = cb_c97
    bad_c97: Callable[[complex, float], int] = cb_d97
    bad_d97: Callable[[complex], float] = cb_b97

def fn98(
    cb_a98: Callable[[float], int],
    cb_b98: Callable[[float], float],
    cb_c98: Callable[[float], complex],
    cb_d98: Callable[[float, float], int],
) -> None:
    ok_a98: Callable[[int], int] = cb_a98
    ok_b98: Callable[[int, float], float] = cb_d98
    bad_a98: Callable[[complex], int] = cb_a98
    bad_b98: Callable[[float], float] = cb_c98
    bad_c98: Callable[[float, complex], int] = cb_d98
    bad_d98: Callable[[int], float] = cb_c98

def fn99(
    cb_a99: Callable[[float], int],
    cb_b99: Callable[[float], float],
    cb_c99: Callable[[float], complex],
    cb_d99: Callable[[float, float], int],
) -> None:
    ok_a99: Callable[[int], float] = cb_a99
    ok_b99: Callable[[float], float] = cb_b99
    bad_a99: Callable[[complex], int] = cb_a99
    bad_b99: Callable[[complex], float] = cb_b99
    bad_c99: Callable[[complex, complex], int] = cb_d99
    bad_d99: Callable[[float, complex], int] = cb_d99

def fn100(
    cb_a100: Callable[[float], int],
    cb_b100: Callable[[float], float],
    cb_c100: Callable[[float], complex],
    cb_d100: Callable[[float, float], int],
) -> None:
    ok_a100: Callable[[float], complex] = cb_c100
    ok_b100: Callable[[int], complex] = cb_c100
    bad_a100: Callable[[float], float] = cb_c100
    bad_b100: Callable[[int], float] = cb_c100
    bad_c100: Callable[[complex, float], int] = cb_d100
    bad_d100: Callable[[complex], float] = cb_b100

def fn101(
    cb_a101: Callable[[float], int],
    cb_b101: Callable[[float], float],
    cb_c101: Callable[[float], complex],
    cb_d101: Callable[[float, float], int],
) -> None:
    ok_a101: Callable[[int], int] = cb_a101
    ok_b101: Callable[[int, float], float] = cb_d101
    bad_a101: Callable[[complex], int] = cb_a101
    bad_b101: Callable[[float], float] = cb_c101
    bad_c101: Callable[[float, complex], int] = cb_d101
    bad_d101: Callable[[int], float] = cb_c101

def fn102(
    cb_a102: Callable[[float], int],
    cb_b102: Callable[[float], float],
    cb_c102: Callable[[float], complex],
    cb_d102: Callable[[float, float], int],
) -> None:
    ok_a102: Callable[[int], float] = cb_a102
    ok_b102: Callable[[float], float] = cb_b102
    bad_a102: Callable[[complex], int] = cb_a102
    bad_b102: Callable[[complex], float] = cb_b102
    bad_c102: Callable[[complex, complex], int] = cb_d102
    bad_d102: Callable[[float, complex], int] = cb_d102

def fn103(
    cb_a103: Callable[[float], int],
    cb_b103: Callable[[float], float],
    cb_c103: Callable[[float], complex],
    cb_d103: Callable[[float, float], int],
) -> None:
    ok_a103: Callable[[float], complex] = cb_c103
    ok_b103: Callable[[int], complex] = cb_c103
    bad_a103: Callable[[float], float] = cb_c103
    bad_b103: Callable[[int], float] = cb_c103
    bad_c103: Callable[[complex, float], int] = cb_d103
    bad_d103: Callable[[complex], float] = cb_b103

def fn104(
    cb_a104: Callable[[float], int],
    cb_b104: Callable[[float], float],
    cb_c104: Callable[[float], complex],
    cb_d104: Callable[[float, float], int],
) -> None:
    ok_a104: Callable[[int], int] = cb_a104
    ok_b104: Callable[[int, float], float] = cb_d104
    bad_a104: Callable[[complex], int] = cb_a104
    bad_b104: Callable[[float], float] = cb_c104
    bad_c104: Callable[[float, complex], int] = cb_d104
    bad_d104: Callable[[int], float] = cb_c104

def fn105(
    cb_a105: Callable[[float], int],
    cb_b105: Callable[[float], float],
    cb_c105: Callable[[float], complex],
    cb_d105: Callable[[float, float], int],
) -> None:
    ok_a105: Callable[[int], float] = cb_a105
    ok_b105: Callable[[float], float] = cb_b105
    bad_a105: Callable[[complex], int] = cb_a105
    bad_b105: Callable[[complex], float] = cb_b105
    bad_c105: Callable[[complex, complex], int] = cb_d105
    bad_d105: Callable[[float, complex], int] = cb_d105

def fn106(
    cb_a106: Callable[[float], int],
    cb_b106: Callable[[float], float],
    cb_c106: Callable[[float], complex],
    cb_d106: Callable[[float, float], int],
) -> None:
    ok_a106: Callable[[float], complex] = cb_c106
    ok_b106: Callable[[int], complex] = cb_c106
    bad_a106: Callable[[float], float] = cb_c106
    bad_b106: Callable[[int], float] = cb_c106
    bad_c106: Callable[[complex, float], int] = cb_d106
    bad_d106: Callable[[complex], float] = cb_b106

def fn107(
    cb_a107: Callable[[float], int],
    cb_b107: Callable[[float], float],
    cb_c107: Callable[[float], complex],
    cb_d107: Callable[[float, float], int],
) -> None:
    ok_a107: Callable[[int], int] = cb_a107
    ok_b107: Callable[[int, float], float] = cb_d107
    bad_a107: Callable[[complex], int] = cb_a107
    bad_b107: Callable[[float], float] = cb_c107
    bad_c107: Callable[[float, complex], int] = cb_d107
    bad_d107: Callable[[int], float] = cb_c107

def fn108(
    cb_a108: Callable[[float], int],
    cb_b108: Callable[[float], float],
    cb_c108: Callable[[float], complex],
    cb_d108: Callable[[float, float], int],
) -> None:
    ok_a108: Callable[[int], float] = cb_a108
    ok_b108: Callable[[float], float] = cb_b108
    bad_a108: Callable[[complex], int] = cb_a108
    bad_b108: Callable[[complex], float] = cb_b108
    bad_c108: Callable[[complex, complex], int] = cb_d108
    bad_d108: Callable[[float, complex], int] = cb_d108

def fn109(
    cb_a109: Callable[[float], int],
    cb_b109: Callable[[float], float],
    cb_c109: Callable[[float], complex],
    cb_d109: Callable[[float, float], int],
) -> None:
    ok_a109: Callable[[float], complex] = cb_c109
    ok_b109: Callable[[int], complex] = cb_c109
    bad_a109: Callable[[float], float] = cb_c109
    bad_b109: Callable[[int], float] = cb_c109
    bad_c109: Callable[[complex, float], int] = cb_d109
    bad_d109: Callable[[complex], float] = cb_b109

def fn110(
    cb_a110: Callable[[float], int],
    cb_b110: Callable[[float], float],
    cb_c110: Callable[[float], complex],
    cb_d110: Callable[[float, float], int],
) -> None:
    ok_a110: Callable[[int], int] = cb_a110
    ok_b110: Callable[[int, float], float] = cb_d110
    bad_a110: Callable[[complex], int] = cb_a110
    bad_b110: Callable[[float], float] = cb_c110
    bad_c110: Callable[[float, complex], int] = cb_d110
    bad_d110: Callable[[int], float] = cb_c110

def fn111(
    cb_a111: Callable[[float], int],
    cb_b111: Callable[[float], float],
    cb_c111: Callable[[float], complex],
    cb_d111: Callable[[float, float], int],
) -> None:
    ok_a111: Callable[[int], float] = cb_a111
    ok_b111: Callable[[float], float] = cb_b111
    bad_a111: Callable[[complex], int] = cb_a111
    bad_b111: Callable[[complex], float] = cb_b111
    bad_c111: Callable[[complex, complex], int] = cb_d111
    bad_d111: Callable[[float, complex], int] = cb_d111

def fn112(
    cb_a112: Callable[[float], int],
    cb_b112: Callable[[float], float],
    cb_c112: Callable[[float], complex],
    cb_d112: Callable[[float, float], int],
) -> None:
    ok_a112: Callable[[float], complex] = cb_c112
    ok_b112: Callable[[int], complex] = cb_c112
    bad_a112: Callable[[float], float] = cb_c112
    bad_b112: Callable[[int], float] = cb_c112
    bad_c112: Callable[[complex, float], int] = cb_d112
    bad_d112: Callable[[complex], float] = cb_b112

def fn113(
    cb_a113: Callable[[float], int],
    cb_b113: Callable[[float], float],
    cb_c113: Callable[[float], complex],
    cb_d113: Callable[[float, float], int],
) -> None:
    ok_a113: Callable[[int], int] = cb_a113
    ok_b113: Callable[[int, float], float] = cb_d113
    bad_a113: Callable[[complex], int] = cb_a113
    bad_b113: Callable[[float], float] = cb_c113
    bad_c113: Callable[[float, complex], int] = cb_d113
    bad_d113: Callable[[int], float] = cb_c113

def fn114(
    cb_a114: Callable[[float], int],
    cb_b114: Callable[[float], float],
    cb_c114: Callable[[float], complex],
    cb_d114: Callable[[float, float], int],
) -> None:
    ok_a114: Callable[[int], float] = cb_a114
    ok_b114: Callable[[float], float] = cb_b114
    bad_a114: Callable[[complex], int] = cb_a114
    bad_b114: Callable[[complex], float] = cb_b114
    bad_c114: Callable[[complex, complex], int] = cb_d114
    bad_d114: Callable[[float, complex], int] = cb_d114

def fn115(
    cb_a115: Callable[[float], int],
    cb_b115: Callable[[float], float],
    cb_c115: Callable[[float], complex],
    cb_d115: Callable[[float, float], int],
) -> None:
    ok_a115: Callable[[float], complex] = cb_c115
    ok_b115: Callable[[int], complex] = cb_c115
    bad_a115: Callable[[float], float] = cb_c115
    bad_b115: Callable[[int], float] = cb_c115
    bad_c115: Callable[[complex, float], int] = cb_d115
    bad_d115: Callable[[complex], float] = cb_b115

def fn116(
    cb_a116: Callable[[float], int],
    cb_b116: Callable[[float], float],
    cb_c116: Callable[[float], complex],
    cb_d116: Callable[[float, float], int],
) -> None:
    ok_a116: Callable[[int], int] = cb_a116
    ok_b116: Callable[[int, float], float] = cb_d116
    bad_a116: Callable[[complex], int] = cb_a116
    bad_b116: Callable[[float], float] = cb_c116
    bad_c116: Callable[[float, complex], int] = cb_d116
    bad_d116: Callable[[int], float] = cb_c116

def fn117(
    cb_a117: Callable[[float], int],
    cb_b117: Callable[[float], float],
    cb_c117: Callable[[float], complex],
    cb_d117: Callable[[float, float], int],
) -> None:
    ok_a117: Callable[[int], float] = cb_a117
    ok_b117: Callable[[float], float] = cb_b117
    bad_a117: Callable[[complex], int] = cb_a117
    bad_b117: Callable[[complex], float] = cb_b117
    bad_c117: Callable[[complex, complex], int] = cb_d117
    bad_d117: Callable[[float, complex], int] = cb_d117

def fn118(
    cb_a118: Callable[[float], int],
    cb_b118: Callable[[float], float],
    cb_c118: Callable[[float], complex],
    cb_d118: Callable[[float, float], int],
) -> None:
    ok_a118: Callable[[float], complex] = cb_c118
    ok_b118: Callable[[int], complex] = cb_c118
    bad_a118: Callable[[float], float] = cb_c118
    bad_b118: Callable[[int], float] = cb_c118
    bad_c118: Callable[[complex, float], int] = cb_d118
    bad_d118: Callable[[complex], float] = cb_b118

def fn119(
    cb_a119: Callable[[float], int],
    cb_b119: Callable[[float], float],
    cb_c119: Callable[[float], complex],
    cb_d119: Callable[[float, float], int],
) -> None:
    ok_a119: Callable[[int], int] = cb_a119
    ok_b119: Callable[[int, float], float] = cb_d119
    bad_a119: Callable[[complex], int] = cb_a119
    bad_b119: Callable[[float], float] = cb_c119
    bad_c119: Callable[[float, complex], int] = cb_d119
    bad_d119: Callable[[int], float] = cb_c119

def fn120(
    cb_a120: Callable[[float], int],
    cb_b120: Callable[[float], float],
    cb_c120: Callable[[float], complex],
    cb_d120: Callable[[float, float], int],
) -> None:
    ok_a120: Callable[[int], float] = cb_a120
    ok_b120: Callable[[float], float] = cb_b120
    bad_a120: Callable[[complex], int] = cb_a120
    bad_b120: Callable[[complex], float] = cb_b120
    bad_c120: Callable[[complex, complex], int] = cb_d120
    bad_d120: Callable[[float, complex], int] = cb_d120

def fn121(
    cb_a121: Callable[[float], int],
    cb_b121: Callable[[float], float],
    cb_c121: Callable[[float], complex],
    cb_d121: Callable[[float, float], int],
) -> None:
    ok_a121: Callable[[float], complex] = cb_c121
    ok_b121: Callable[[int], complex] = cb_c121
    bad_a121: Callable[[float], float] = cb_c121
    bad_b121: Callable[[int], float] = cb_c121
    bad_c121: Callable[[complex, float], int] = cb_d121
    bad_d121: Callable[[complex], float] = cb_b121

def fn122(
    cb_a122: Callable[[float], int],
    cb_b122: Callable[[float], float],
    cb_c122: Callable[[float], complex],
    cb_d122: Callable[[float, float], int],
) -> None:
    ok_a122: Callable[[int], int] = cb_a122
    ok_b122: Callable[[int, float], float] = cb_d122
    bad_a122: Callable[[complex], int] = cb_a122
    bad_b122: Callable[[float], float] = cb_c122
    bad_c122: Callable[[float, complex], int] = cb_d122
    bad_d122: Callable[[int], float] = cb_c122

def fn123(
    cb_a123: Callable[[float], int],
    cb_b123: Callable[[float], float],
    cb_c123: Callable[[float], complex],
    cb_d123: Callable[[float, float], int],
) -> None:
    ok_a123: Callable[[int], float] = cb_a123
    ok_b123: Callable[[float], float] = cb_b123
    bad_a123: Callable[[complex], int] = cb_a123
    bad_b123: Callable[[complex], float] = cb_b123
    bad_c123: Callable[[complex, complex], int] = cb_d123
    bad_d123: Callable[[float, complex], int] = cb_d123

def fn124(
    cb_a124: Callable[[float], int],
    cb_b124: Callable[[float], float],
    cb_c124: Callable[[float], complex],
    cb_d124: Callable[[float, float], int],
) -> None:
    ok_a124: Callable[[float], complex] = cb_c124
    ok_b124: Callable[[int], complex] = cb_c124
    bad_a124: Callable[[float], float] = cb_c124
    bad_b124: Callable[[int], float] = cb_c124
    bad_c124: Callable[[complex, float], int] = cb_d124
    bad_d124: Callable[[complex], float] = cb_b124

def fn125(
    cb_a125: Callable[[float], int],
    cb_b125: Callable[[float], float],
    cb_c125: Callable[[float], complex],
    cb_d125: Callable[[float, float], int],
) -> None:
    ok_a125: Callable[[int], int] = cb_a125
    ok_b125: Callable[[int, float], float] = cb_d125
    bad_a125: Callable[[complex], int] = cb_a125
    bad_b125: Callable[[float], float] = cb_c125
    bad_c125: Callable[[float, complex], int] = cb_d125
    bad_d125: Callable[[int], float] = cb_c125

def fn126(
    cb_a126: Callable[[float], int],
    cb_b126: Callable[[float], float],
    cb_c126: Callable[[float], complex],
    cb_d126: Callable[[float, float], int],
) -> None:
    ok_a126: Callable[[int], float] = cb_a126
    ok_b126: Callable[[float], float] = cb_b126
    bad_a126: Callable[[complex], int] = cb_a126
    bad_b126: Callable[[complex], float] = cb_b126
    bad_c126: Callable[[complex, complex], int] = cb_d126
    bad_d126: Callable[[float, complex], int] = cb_d126

def fn127(
    cb_a127: Callable[[float], int],
    cb_b127: Callable[[float], float],
    cb_c127: Callable[[float], complex],
    cb_d127: Callable[[float, float], int],
) -> None:
    ok_a127: Callable[[float], complex] = cb_c127
    ok_b127: Callable[[int], complex] = cb_c127
    bad_a127: Callable[[float], float] = cb_c127
    bad_b127: Callable[[int], float] = cb_c127
    bad_c127: Callable[[complex, float], int] = cb_d127
    bad_d127: Callable[[complex], float] = cb_b127

def fn128(
    cb_a128: Callable[[float], int],
    cb_b128: Callable[[float], float],
    cb_c128: Callable[[float], complex],
    cb_d128: Callable[[float, float], int],
) -> None:
    ok_a128: Callable[[int], int] = cb_a128
    ok_b128: Callable[[int, float], float] = cb_d128
    bad_a128: Callable[[complex], int] = cb_a128
    bad_b128: Callable[[float], float] = cb_c128
    bad_c128: Callable[[float, complex], int] = cb_d128
    bad_d128: Callable[[int], float] = cb_c128

def fn129(
    cb_a129: Callable[[float], int],
    cb_b129: Callable[[float], float],
    cb_c129: Callable[[float], complex],
    cb_d129: Callable[[float, float], int],
) -> None:
    ok_a129: Callable[[int], float] = cb_a129
    ok_b129: Callable[[float], float] = cb_b129
    bad_a129: Callable[[complex], int] = cb_a129
    bad_b129: Callable[[complex], float] = cb_b129
    bad_c129: Callable[[complex, complex], int] = cb_d129
    bad_d129: Callable[[float, complex], int] = cb_d129

def fn130(
    cb_a130: Callable[[float], int],
    cb_b130: Callable[[float], float],
    cb_c130: Callable[[float], complex],
    cb_d130: Callable[[float, float], int],
) -> None:
    ok_a130: Callable[[float], complex] = cb_c130
    ok_b130: Callable[[int], complex] = cb_c130
    bad_a130: Callable[[float], float] = cb_c130
    bad_b130: Callable[[int], float] = cb_c130
    bad_c130: Callable[[complex, float], int] = cb_d130
    bad_d130: Callable[[complex], float] = cb_b130

def fn131(
    cb_a131: Callable[[float], int],
    cb_b131: Callable[[float], float],
    cb_c131: Callable[[float], complex],
    cb_d131: Callable[[float, float], int],
) -> None:
    ok_a131: Callable[[int], int] = cb_a131
    ok_b131: Callable[[int, float], float] = cb_d131
    bad_a131: Callable[[complex], int] = cb_a131
    bad_b131: Callable[[float], float] = cb_c131
    bad_c131: Callable[[float, complex], int] = cb_d131
    bad_d131: Callable[[int], float] = cb_c131

def fn132(
    cb_a132: Callable[[float], int],
    cb_b132: Callable[[float], float],
    cb_c132: Callable[[float], complex],
    cb_d132: Callable[[float, float], int],
) -> None:
    ok_a132: Callable[[int], float] = cb_a132
    ok_b132: Callable[[float], float] = cb_b132
    bad_a132: Callable[[complex], int] = cb_a132
    bad_b132: Callable[[complex], float] = cb_b132
    bad_c132: Callable[[complex, complex], int] = cb_d132
    bad_d132: Callable[[float, complex], int] = cb_d132

def fn133(
    cb_a133: Callable[[float], int],
    cb_b133: Callable[[float], float],
    cb_c133: Callable[[float], complex],
    cb_d133: Callable[[float, float], int],
) -> None:
    ok_a133: Callable[[float], complex] = cb_c133
    ok_b133: Callable[[int], complex] = cb_c133
    bad_a133: Callable[[float], float] = cb_c133
    bad_b133: Callable[[int], float] = cb_c133
    bad_c133: Callable[[complex, float], int] = cb_d133
    bad_d133: Callable[[complex], float] = cb_b133

def fn134(
    cb_a134: Callable[[float], int],
    cb_b134: Callable[[float], float],
    cb_c134: Callable[[float], complex],
    cb_d134: Callable[[float, float], int],
) -> None:
    ok_a134: Callable[[int], int] = cb_a134
    ok_b134: Callable[[int, float], float] = cb_d134
    bad_a134: Callable[[complex], int] = cb_a134
    bad_b134: Callable[[float], float] = cb_c134
    bad_c134: Callable[[float, complex], int] = cb_d134
    bad_d134: Callable[[int], float] = cb_c134

def fn135(
    cb_a135: Callable[[float], int],
    cb_b135: Callable[[float], float],
    cb_c135: Callable[[float], complex],
    cb_d135: Callable[[float, float], int],
) -> None:
    ok_a135: Callable[[int], float] = cb_a135
    ok_b135: Callable[[float], float] = cb_b135
    bad_a135: Callable[[complex], int] = cb_a135
    bad_b135: Callable[[complex], float] = cb_b135
    bad_c135: Callable[[complex, complex], int] = cb_d135
    bad_d135: Callable[[float, complex], int] = cb_d135

def fn136(
    cb_a136: Callable[[float], int],
    cb_b136: Callable[[float], float],
    cb_c136: Callable[[float], complex],
    cb_d136: Callable[[float, float], int],
) -> None:
    ok_a136: Callable[[float], complex] = cb_c136
    ok_b136: Callable[[int], complex] = cb_c136
    bad_a136: Callable[[float], float] = cb_c136
    bad_b136: Callable[[int], float] = cb_c136
    bad_c136: Callable[[complex, float], int] = cb_d136
    bad_d136: Callable[[complex], float] = cb_b136

def fn137(
    cb_a137: Callable[[float], int],
    cb_b137: Callable[[float], float],
    cb_c137: Callable[[float], complex],
    cb_d137: Callable[[float, float], int],
) -> None:
    ok_a137: Callable[[int], int] = cb_a137
    ok_b137: Callable[[int, float], float] = cb_d137
    bad_a137: Callable[[complex], int] = cb_a137
    bad_b137: Callable[[float], float] = cb_c137
    bad_c137: Callable[[float, complex], int] = cb_d137
    bad_d137: Callable[[int], float] = cb_c137

def fn138(
    cb_a138: Callable[[float], int],
    cb_b138: Callable[[float], float],
    cb_c138: Callable[[float], complex],
    cb_d138: Callable[[float, float], int],
) -> None:
    ok_a138: Callable[[int], float] = cb_a138
    ok_b138: Callable[[float], float] = cb_b138
    bad_a138: Callable[[complex], int] = cb_a138
    bad_b138: Callable[[complex], float] = cb_b138
    bad_c138: Callable[[complex, complex], int] = cb_d138
    bad_d138: Callable[[float, complex], int] = cb_d138

def fn139(
    cb_a139: Callable[[float], int],
    cb_b139: Callable[[float], float],
    cb_c139: Callable[[float], complex],
    cb_d139: Callable[[float, float], int],
) -> None:
    ok_a139: Callable[[float], complex] = cb_c139
    ok_b139: Callable[[int], complex] = cb_c139
    bad_a139: Callable[[float], float] = cb_c139
    bad_b139: Callable[[int], float] = cb_c139
    bad_c139: Callable[[complex, float], int] = cb_d139
    bad_d139: Callable[[complex], float] = cb_b139

def fn140(
    cb_a140: Callable[[float], int],
    cb_b140: Callable[[float], float],
    cb_c140: Callable[[float], complex],
    cb_d140: Callable[[float, float], int],
) -> None:
    ok_a140: Callable[[int], int] = cb_a140
    ok_b140: Callable[[int, float], float] = cb_d140
    bad_a140: Callable[[complex], int] = cb_a140
    bad_b140: Callable[[float], float] = cb_c140
    bad_c140: Callable[[float, complex], int] = cb_d140
    bad_d140: Callable[[int], float] = cb_c140

def fn141(
    cb_a141: Callable[[float], int],
    cb_b141: Callable[[float], float],
    cb_c141: Callable[[float], complex],
    cb_d141: Callable[[float, float], int],
) -> None:
    ok_a141: Callable[[int], float] = cb_a141
    ok_b141: Callable[[float], float] = cb_b141
    bad_a141: Callable[[complex], int] = cb_a141
    bad_b141: Callable[[complex], float] = cb_b141
    bad_c141: Callable[[complex, complex], int] = cb_d141
    bad_d141: Callable[[float, complex], int] = cb_d141

def fn142(
    cb_a142: Callable[[float], int],
    cb_b142: Callable[[float], float],
    cb_c142: Callable[[float], complex],
    cb_d142: Callable[[float, float], int],
) -> None:
    ok_a142: Callable[[float], complex] = cb_c142
    ok_b142: Callable[[int], complex] = cb_c142
    bad_a142: Callable[[float], float] = cb_c142
    bad_b142: Callable[[int], float] = cb_c142
    bad_c142: Callable[[complex, float], int] = cb_d142
    bad_d142: Callable[[complex], float] = cb_b142

def fn143(
    cb_a143: Callable[[float], int],
    cb_b143: Callable[[float], float],
    cb_c143: Callable[[float], complex],
    cb_d143: Callable[[float, float], int],
) -> None:
    ok_a143: Callable[[int], int] = cb_a143
    ok_b143: Callable[[int, float], float] = cb_d143
    bad_a143: Callable[[complex], int] = cb_a143
    bad_b143: Callable[[float], float] = cb_c143
    bad_c143: Callable[[float, complex], int] = cb_d143
    bad_d143: Callable[[int], float] = cb_c143

def fn144(
    cb_a144: Callable[[float], int],
    cb_b144: Callable[[float], float],
    cb_c144: Callable[[float], complex],
    cb_d144: Callable[[float, float], int],
) -> None:
    ok_a144: Callable[[int], float] = cb_a144
    ok_b144: Callable[[float], float] = cb_b144
    bad_a144: Callable[[complex], int] = cb_a144
    bad_b144: Callable[[complex], float] = cb_b144
    bad_c144: Callable[[complex, complex], int] = cb_d144
    bad_d144: Callable[[float, complex], int] = cb_d144

def fn145(
    cb_a145: Callable[[float], int],
    cb_b145: Callable[[float], float],
    cb_c145: Callable[[float], complex],
    cb_d145: Callable[[float, float], int],
) -> None:
    ok_a145: Callable[[float], complex] = cb_c145
    ok_b145: Callable[[int], complex] = cb_c145
    bad_a145: Callable[[float], float] = cb_c145
    bad_b145: Callable[[int], float] = cb_c145
    bad_c145: Callable[[complex, float], int] = cb_d145
    bad_d145: Callable[[complex], float] = cb_b145

def fn146(
    cb_a146: Callable[[float], int],
    cb_b146: Callable[[float], float],
    cb_c146: Callable[[float], complex],
    cb_d146: Callable[[float, float], int],
) -> None:
    ok_a146: Callable[[int], int] = cb_a146
    ok_b146: Callable[[int, float], float] = cb_d146
    bad_a146: Callable[[complex], int] = cb_a146
    bad_b146: Callable[[float], float] = cb_c146
    bad_c146: Callable[[float, complex], int] = cb_d146
    bad_d146: Callable[[int], float] = cb_c146

def fn147(
    cb_a147: Callable[[float], int],
    cb_b147: Callable[[float], float],
    cb_c147: Callable[[float], complex],
    cb_d147: Callable[[float, float], int],
) -> None:
    ok_a147: Callable[[int], float] = cb_a147
    ok_b147: Callable[[float], float] = cb_b147
    bad_a147: Callable[[complex], int] = cb_a147
    bad_b147: Callable[[complex], float] = cb_b147
    bad_c147: Callable[[complex, complex], int] = cb_d147
    bad_d147: Callable[[float, complex], int] = cb_d147

def fn148(
    cb_a148: Callable[[float], int],
    cb_b148: Callable[[float], float],
    cb_c148: Callable[[float], complex],
    cb_d148: Callable[[float, float], int],
) -> None:
    ok_a148: Callable[[float], complex] = cb_c148
    ok_b148: Callable[[int], complex] = cb_c148
    bad_a148: Callable[[float], float] = cb_c148
    bad_b148: Callable[[int], float] = cb_c148
    bad_c148: Callable[[complex, float], int] = cb_d148
    bad_d148: Callable[[complex], float] = cb_b148

def fn149(
    cb_a149: Callable[[float], int],
    cb_b149: Callable[[float], float],
    cb_c149: Callable[[float], complex],
    cb_d149: Callable[[float, float], int],
) -> None:
    ok_a149: Callable[[int], int] = cb_a149
    ok_b149: Callable[[int, float], float] = cb_d149
    bad_a149: Callable[[complex], int] = cb_a149
    bad_b149: Callable[[float], float] = cb_c149
    bad_c149: Callable[[float, complex], int] = cb_d149
    bad_d149: Callable[[int], float] = cb_c149
