# Benchmark stress fixture for `overloads_evaluation`: every call in the
# check functions passes a union-typed argument where one union member
# matches no overload signature after argument type expansion.
from typing import Union, overload
@overload
def fn0(x: int, y: str, z: int) -> str: ...
@overload
def fn0(x: int, y: int, z: int) -> int: ...
def fn0(x: int, y: int | str, z: int) -> int | str:
    return 1
def check0(a: int | str, b: str | int, c: bytes | int) -> None:
    fn0(a, a, 1)
    fn0(b, b, 2)
    fn0(c, c, 3)
@overload
def fn1(x: int) -> int: ...
@overload
def fn1(x: str) -> str: ...
def fn1(x: int | str) -> int | str:
    return 1
def check1(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn1(a)
    fn1(b)
    fn1(c)
@overload
def fn2(x: str, y: int) -> str: ...
@overload
def fn2(x: bytes, y: int) -> bytes: ...
def fn2(x: str | bytes, y: int) -> str | bytes:
    return ""
def check2(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn2(a, 1)
    fn2(b, 2)
    fn2(c, 3)
@overload
def fn3(x: int, y: str, z: int) -> str: ...
@overload
def fn3(x: int, y: int, z: int) -> int: ...
def fn3(x: int, y: int | str, z: int) -> int | str:
    return 1
def check3(a: int | str, b: str | int, c: bytes | int) -> None:
    fn3(a, a, 1)
    fn3(b, b, 2)
    fn3(c, c, 3)
@overload
def fn4(x: int) -> int: ...
@overload
def fn4(x: str) -> str: ...
def fn4(x: int | str) -> int | str:
    return 1
def check4(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn4(a)
    fn4(b)
    fn4(c)
@overload
def fn5(x: str, y: int) -> str: ...
@overload
def fn5(x: bytes, y: int) -> bytes: ...
def fn5(x: str | bytes, y: int) -> str | bytes:
    return ""
def check5(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn5(a, 1)
    fn5(b, 2)
    fn5(c, 3)
@overload
def fn6(x: int, y: str, z: int) -> str: ...
@overload
def fn6(x: int, y: int, z: int) -> int: ...
def fn6(x: int, y: int | str, z: int) -> int | str:
    return 1
def check6(a: int | str, b: str | int, c: bytes | int) -> None:
    fn6(a, a, 1)
    fn6(b, b, 2)
    fn6(c, c, 3)
@overload
def fn7(x: int) -> int: ...
@overload
def fn7(x: str) -> str: ...
def fn7(x: int | str) -> int | str:
    return 1
def check7(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn7(a)
    fn7(b)
    fn7(c)
@overload
def fn8(x: str, y: int) -> str: ...
@overload
def fn8(x: bytes, y: int) -> bytes: ...
def fn8(x: str | bytes, y: int) -> str | bytes:
    return ""
def check8(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn8(a, 1)
    fn8(b, 2)
    fn8(c, 3)
@overload
def fn9(x: int, y: str, z: int) -> str: ...
@overload
def fn9(x: int, y: int, z: int) -> int: ...
def fn9(x: int, y: int | str, z: int) -> int | str:
    return 1
def check9(a: int | str, b: str | int, c: bytes | int) -> None:
    fn9(a, a, 1)
    fn9(b, b, 2)
    fn9(c, c, 3)
@overload
def fn10(x: int) -> int: ...
@overload
def fn10(x: str) -> str: ...
def fn10(x: int | str) -> int | str:
    return 1
def check10(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn10(a)
    fn10(b)
    fn10(c)
@overload
def fn11(x: str, y: int) -> str: ...
@overload
def fn11(x: bytes, y: int) -> bytes: ...
def fn11(x: str | bytes, y: int) -> str | bytes:
    return ""
def check11(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn11(a, 1)
    fn11(b, 2)
    fn11(c, 3)
@overload
def fn12(x: int, y: str, z: int) -> str: ...
@overload
def fn12(x: int, y: int, z: int) -> int: ...
def fn12(x: int, y: int | str, z: int) -> int | str:
    return 1
def check12(a: int | str, b: str | int, c: bytes | int) -> None:
    fn12(a, a, 1)
    fn12(b, b, 2)
    fn12(c, c, 3)
@overload
def fn13(x: int) -> int: ...
@overload
def fn13(x: str) -> str: ...
def fn13(x: int | str) -> int | str:
    return 1
def check13(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn13(a)
    fn13(b)
    fn13(c)
@overload
def fn14(x: str, y: int) -> str: ...
@overload
def fn14(x: bytes, y: int) -> bytes: ...
def fn14(x: str | bytes, y: int) -> str | bytes:
    return ""
def check14(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn14(a, 1)
    fn14(b, 2)
    fn14(c, 3)
@overload
def fn15(x: int, y: str, z: int) -> str: ...
@overload
def fn15(x: int, y: int, z: int) -> int: ...
def fn15(x: int, y: int | str, z: int) -> int | str:
    return 1
def check15(a: int | str, b: str | int, c: bytes | int) -> None:
    fn15(a, a, 1)
    fn15(b, b, 2)
    fn15(c, c, 3)
@overload
def fn16(x: int) -> int: ...
@overload
def fn16(x: str) -> str: ...
def fn16(x: int | str) -> int | str:
    return 1
def check16(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn16(a)
    fn16(b)
    fn16(c)
@overload
def fn17(x: str, y: int) -> str: ...
@overload
def fn17(x: bytes, y: int) -> bytes: ...
def fn17(x: str | bytes, y: int) -> str | bytes:
    return ""
def check17(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn17(a, 1)
    fn17(b, 2)
    fn17(c, 3)
@overload
def fn18(x: int, y: str, z: int) -> str: ...
@overload
def fn18(x: int, y: int, z: int) -> int: ...
def fn18(x: int, y: int | str, z: int) -> int | str:
    return 1
def check18(a: int | str, b: str | int, c: bytes | int) -> None:
    fn18(a, a, 1)
    fn18(b, b, 2)
    fn18(c, c, 3)
@overload
def fn19(x: int) -> int: ...
@overload
def fn19(x: str) -> str: ...
def fn19(x: int | str) -> int | str:
    return 1
def check19(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn19(a)
    fn19(b)
    fn19(c)
@overload
def fn20(x: str, y: int) -> str: ...
@overload
def fn20(x: bytes, y: int) -> bytes: ...
def fn20(x: str | bytes, y: int) -> str | bytes:
    return ""
def check20(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn20(a, 1)
    fn20(b, 2)
    fn20(c, 3)
@overload
def fn21(x: int, y: str, z: int) -> str: ...
@overload
def fn21(x: int, y: int, z: int) -> int: ...
def fn21(x: int, y: int | str, z: int) -> int | str:
    return 1
def check21(a: int | str, b: str | int, c: bytes | int) -> None:
    fn21(a, a, 1)
    fn21(b, b, 2)
    fn21(c, c, 3)
@overload
def fn22(x: int) -> int: ...
@overload
def fn22(x: str) -> str: ...
def fn22(x: int | str) -> int | str:
    return 1
def check22(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn22(a)
    fn22(b)
    fn22(c)
@overload
def fn23(x: str, y: int) -> str: ...
@overload
def fn23(x: bytes, y: int) -> bytes: ...
def fn23(x: str | bytes, y: int) -> str | bytes:
    return ""
def check23(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn23(a, 1)
    fn23(b, 2)
    fn23(c, 3)
@overload
def fn24(x: int, y: str, z: int) -> str: ...
@overload
def fn24(x: int, y: int, z: int) -> int: ...
def fn24(x: int, y: int | str, z: int) -> int | str:
    return 1
def check24(a: int | str, b: str | int, c: bytes | int) -> None:
    fn24(a, a, 1)
    fn24(b, b, 2)
    fn24(c, c, 3)
@overload
def fn25(x: int) -> int: ...
@overload
def fn25(x: str) -> str: ...
def fn25(x: int | str) -> int | str:
    return 1
def check25(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn25(a)
    fn25(b)
    fn25(c)
@overload
def fn26(x: str, y: int) -> str: ...
@overload
def fn26(x: bytes, y: int) -> bytes: ...
def fn26(x: str | bytes, y: int) -> str | bytes:
    return ""
def check26(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn26(a, 1)
    fn26(b, 2)
    fn26(c, 3)
@overload
def fn27(x: int, y: str, z: int) -> str: ...
@overload
def fn27(x: int, y: int, z: int) -> int: ...
def fn27(x: int, y: int | str, z: int) -> int | str:
    return 1
def check27(a: int | str, b: str | int, c: bytes | int) -> None:
    fn27(a, a, 1)
    fn27(b, b, 2)
    fn27(c, c, 3)
@overload
def fn28(x: int) -> int: ...
@overload
def fn28(x: str) -> str: ...
def fn28(x: int | str) -> int | str:
    return 1
def check28(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn28(a)
    fn28(b)
    fn28(c)
@overload
def fn29(x: str, y: int) -> str: ...
@overload
def fn29(x: bytes, y: int) -> bytes: ...
def fn29(x: str | bytes, y: int) -> str | bytes:
    return ""
def check29(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn29(a, 1)
    fn29(b, 2)
    fn29(c, 3)
@overload
def fn30(x: int, y: str, z: int) -> str: ...
@overload
def fn30(x: int, y: int, z: int) -> int: ...
def fn30(x: int, y: int | str, z: int) -> int | str:
    return 1
def check30(a: int | str, b: str | int, c: bytes | int) -> None:
    fn30(a, a, 1)
    fn30(b, b, 2)
    fn30(c, c, 3)
@overload
def fn31(x: int) -> int: ...
@overload
def fn31(x: str) -> str: ...
def fn31(x: int | str) -> int | str:
    return 1
def check31(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn31(a)
    fn31(b)
    fn31(c)
@overload
def fn32(x: str, y: int) -> str: ...
@overload
def fn32(x: bytes, y: int) -> bytes: ...
def fn32(x: str | bytes, y: int) -> str | bytes:
    return ""
def check32(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn32(a, 1)
    fn32(b, 2)
    fn32(c, 3)
@overload
def fn33(x: int, y: str, z: int) -> str: ...
@overload
def fn33(x: int, y: int, z: int) -> int: ...
def fn33(x: int, y: int | str, z: int) -> int | str:
    return 1
def check33(a: int | str, b: str | int, c: bytes | int) -> None:
    fn33(a, a, 1)
    fn33(b, b, 2)
    fn33(c, c, 3)
@overload
def fn34(x: int) -> int: ...
@overload
def fn34(x: str) -> str: ...
def fn34(x: int | str) -> int | str:
    return 1
def check34(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn34(a)
    fn34(b)
    fn34(c)
@overload
def fn35(x: str, y: int) -> str: ...
@overload
def fn35(x: bytes, y: int) -> bytes: ...
def fn35(x: str | bytes, y: int) -> str | bytes:
    return ""
def check35(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn35(a, 1)
    fn35(b, 2)
    fn35(c, 3)
@overload
def fn36(x: int, y: str, z: int) -> str: ...
@overload
def fn36(x: int, y: int, z: int) -> int: ...
def fn36(x: int, y: int | str, z: int) -> int | str:
    return 1
def check36(a: int | str, b: str | int, c: bytes | int) -> None:
    fn36(a, a, 1)
    fn36(b, b, 2)
    fn36(c, c, 3)
@overload
def fn37(x: int) -> int: ...
@overload
def fn37(x: str) -> str: ...
def fn37(x: int | str) -> int | str:
    return 1
def check37(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn37(a)
    fn37(b)
    fn37(c)
@overload
def fn38(x: str, y: int) -> str: ...
@overload
def fn38(x: bytes, y: int) -> bytes: ...
def fn38(x: str | bytes, y: int) -> str | bytes:
    return ""
def check38(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn38(a, 1)
    fn38(b, 2)
    fn38(c, 3)
@overload
def fn39(x: int, y: str, z: int) -> str: ...
@overload
def fn39(x: int, y: int, z: int) -> int: ...
def fn39(x: int, y: int | str, z: int) -> int | str:
    return 1
def check39(a: int | str, b: str | int, c: bytes | int) -> None:
    fn39(a, a, 1)
    fn39(b, b, 2)
    fn39(c, c, 3)
@overload
def fn40(x: int) -> int: ...
@overload
def fn40(x: str) -> str: ...
def fn40(x: int | str) -> int | str:
    return 1
def check40(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn40(a)
    fn40(b)
    fn40(c)
@overload
def fn41(x: str, y: int) -> str: ...
@overload
def fn41(x: bytes, y: int) -> bytes: ...
def fn41(x: str | bytes, y: int) -> str | bytes:
    return ""
def check41(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn41(a, 1)
    fn41(b, 2)
    fn41(c, 3)
@overload
def fn42(x: int, y: str, z: int) -> str: ...
@overload
def fn42(x: int, y: int, z: int) -> int: ...
def fn42(x: int, y: int | str, z: int) -> int | str:
    return 1
def check42(a: int | str, b: str | int, c: bytes | int) -> None:
    fn42(a, a, 1)
    fn42(b, b, 2)
    fn42(c, c, 3)
@overload
def fn43(x: int) -> int: ...
@overload
def fn43(x: str) -> str: ...
def fn43(x: int | str) -> int | str:
    return 1
def check43(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn43(a)
    fn43(b)
    fn43(c)
@overload
def fn44(x: str, y: int) -> str: ...
@overload
def fn44(x: bytes, y: int) -> bytes: ...
def fn44(x: str | bytes, y: int) -> str | bytes:
    return ""
def check44(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn44(a, 1)
    fn44(b, 2)
    fn44(c, 3)
@overload
def fn45(x: int, y: str, z: int) -> str: ...
@overload
def fn45(x: int, y: int, z: int) -> int: ...
def fn45(x: int, y: int | str, z: int) -> int | str:
    return 1
def check45(a: int | str, b: str | int, c: bytes | int) -> None:
    fn45(a, a, 1)
    fn45(b, b, 2)
    fn45(c, c, 3)
@overload
def fn46(x: int) -> int: ...
@overload
def fn46(x: str) -> str: ...
def fn46(x: int | str) -> int | str:
    return 1
def check46(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn46(a)
    fn46(b)
    fn46(c)
@overload
def fn47(x: str, y: int) -> str: ...
@overload
def fn47(x: bytes, y: int) -> bytes: ...
def fn47(x: str | bytes, y: int) -> str | bytes:
    return ""
def check47(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn47(a, 1)
    fn47(b, 2)
    fn47(c, 3)
@overload
def fn48(x: int, y: str, z: int) -> str: ...
@overload
def fn48(x: int, y: int, z: int) -> int: ...
def fn48(x: int, y: int | str, z: int) -> int | str:
    return 1
def check48(a: int | str, b: str | int, c: bytes | int) -> None:
    fn48(a, a, 1)
    fn48(b, b, 2)
    fn48(c, c, 3)
@overload
def fn49(x: int) -> int: ...
@overload
def fn49(x: str) -> str: ...
def fn49(x: int | str) -> int | str:
    return 1
def check49(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn49(a)
    fn49(b)
    fn49(c)
@overload
def fn50(x: str, y: int) -> str: ...
@overload
def fn50(x: bytes, y: int) -> bytes: ...
def fn50(x: str | bytes, y: int) -> str | bytes:
    return ""
def check50(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn50(a, 1)
    fn50(b, 2)
    fn50(c, 3)
@overload
def fn51(x: int, y: str, z: int) -> str: ...
@overload
def fn51(x: int, y: int, z: int) -> int: ...
def fn51(x: int, y: int | str, z: int) -> int | str:
    return 1
def check51(a: int | str, b: str | int, c: bytes | int) -> None:
    fn51(a, a, 1)
    fn51(b, b, 2)
    fn51(c, c, 3)
@overload
def fn52(x: int) -> int: ...
@overload
def fn52(x: str) -> str: ...
def fn52(x: int | str) -> int | str:
    return 1
def check52(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn52(a)
    fn52(b)
    fn52(c)
@overload
def fn53(x: str, y: int) -> str: ...
@overload
def fn53(x: bytes, y: int) -> bytes: ...
def fn53(x: str | bytes, y: int) -> str | bytes:
    return ""
def check53(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn53(a, 1)
    fn53(b, 2)
    fn53(c, 3)
@overload
def fn54(x: int, y: str, z: int) -> str: ...
@overload
def fn54(x: int, y: int, z: int) -> int: ...
def fn54(x: int, y: int | str, z: int) -> int | str:
    return 1
def check54(a: int | str, b: str | int, c: bytes | int) -> None:
    fn54(a, a, 1)
    fn54(b, b, 2)
    fn54(c, c, 3)
@overload
def fn55(x: int) -> int: ...
@overload
def fn55(x: str) -> str: ...
def fn55(x: int | str) -> int | str:
    return 1
def check55(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn55(a)
    fn55(b)
    fn55(c)
@overload
def fn56(x: str, y: int) -> str: ...
@overload
def fn56(x: bytes, y: int) -> bytes: ...
def fn56(x: str | bytes, y: int) -> str | bytes:
    return ""
def check56(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn56(a, 1)
    fn56(b, 2)
    fn56(c, 3)
@overload
def fn57(x: int, y: str, z: int) -> str: ...
@overload
def fn57(x: int, y: int, z: int) -> int: ...
def fn57(x: int, y: int | str, z: int) -> int | str:
    return 1
def check57(a: int | str, b: str | int, c: bytes | int) -> None:
    fn57(a, a, 1)
    fn57(b, b, 2)
    fn57(c, c, 3)
@overload
def fn58(x: int) -> int: ...
@overload
def fn58(x: str) -> str: ...
def fn58(x: int | str) -> int | str:
    return 1
def check58(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn58(a)
    fn58(b)
    fn58(c)
@overload
def fn59(x: str, y: int) -> str: ...
@overload
def fn59(x: bytes, y: int) -> bytes: ...
def fn59(x: str | bytes, y: int) -> str | bytes:
    return ""
def check59(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn59(a, 1)
    fn59(b, 2)
    fn59(c, 3)
@overload
def fn60(x: int, y: str, z: int) -> str: ...
@overload
def fn60(x: int, y: int, z: int) -> int: ...
def fn60(x: int, y: int | str, z: int) -> int | str:
    return 1
def check60(a: int | str, b: str | int, c: bytes | int) -> None:
    fn60(a, a, 1)
    fn60(b, b, 2)
    fn60(c, c, 3)
@overload
def fn61(x: int) -> int: ...
@overload
def fn61(x: str) -> str: ...
def fn61(x: int | str) -> int | str:
    return 1
def check61(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn61(a)
    fn61(b)
    fn61(c)
@overload
def fn62(x: str, y: int) -> str: ...
@overload
def fn62(x: bytes, y: int) -> bytes: ...
def fn62(x: str | bytes, y: int) -> str | bytes:
    return ""
def check62(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn62(a, 1)
    fn62(b, 2)
    fn62(c, 3)
@overload
def fn63(x: int, y: str, z: int) -> str: ...
@overload
def fn63(x: int, y: int, z: int) -> int: ...
def fn63(x: int, y: int | str, z: int) -> int | str:
    return 1
def check63(a: int | str, b: str | int, c: bytes | int) -> None:
    fn63(a, a, 1)
    fn63(b, b, 2)
    fn63(c, c, 3)
@overload
def fn64(x: int) -> int: ...
@overload
def fn64(x: str) -> str: ...
def fn64(x: int | str) -> int | str:
    return 1
def check64(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn64(a)
    fn64(b)
    fn64(c)
@overload
def fn65(x: str, y: int) -> str: ...
@overload
def fn65(x: bytes, y: int) -> bytes: ...
def fn65(x: str | bytes, y: int) -> str | bytes:
    return ""
def check65(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn65(a, 1)
    fn65(b, 2)
    fn65(c, 3)
@overload
def fn66(x: int, y: str, z: int) -> str: ...
@overload
def fn66(x: int, y: int, z: int) -> int: ...
def fn66(x: int, y: int | str, z: int) -> int | str:
    return 1
def check66(a: int | str, b: str | int, c: bytes | int) -> None:
    fn66(a, a, 1)
    fn66(b, b, 2)
    fn66(c, c, 3)
@overload
def fn67(x: int) -> int: ...
@overload
def fn67(x: str) -> str: ...
def fn67(x: int | str) -> int | str:
    return 1
def check67(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn67(a)
    fn67(b)
    fn67(c)
@overload
def fn68(x: str, y: int) -> str: ...
@overload
def fn68(x: bytes, y: int) -> bytes: ...
def fn68(x: str | bytes, y: int) -> str | bytes:
    return ""
def check68(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn68(a, 1)
    fn68(b, 2)
    fn68(c, 3)
@overload
def fn69(x: int, y: str, z: int) -> str: ...
@overload
def fn69(x: int, y: int, z: int) -> int: ...
def fn69(x: int, y: int | str, z: int) -> int | str:
    return 1
def check69(a: int | str, b: str | int, c: bytes | int) -> None:
    fn69(a, a, 1)
    fn69(b, b, 2)
    fn69(c, c, 3)
@overload
def fn70(x: int) -> int: ...
@overload
def fn70(x: str) -> str: ...
def fn70(x: int | str) -> int | str:
    return 1
def check70(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn70(a)
    fn70(b)
    fn70(c)
@overload
def fn71(x: str, y: int) -> str: ...
@overload
def fn71(x: bytes, y: int) -> bytes: ...
def fn71(x: str | bytes, y: int) -> str | bytes:
    return ""
def check71(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn71(a, 1)
    fn71(b, 2)
    fn71(c, 3)
@overload
def fn72(x: int, y: str, z: int) -> str: ...
@overload
def fn72(x: int, y: int, z: int) -> int: ...
def fn72(x: int, y: int | str, z: int) -> int | str:
    return 1
def check72(a: int | str, b: str | int, c: bytes | int) -> None:
    fn72(a, a, 1)
    fn72(b, b, 2)
    fn72(c, c, 3)
@overload
def fn73(x: int) -> int: ...
@overload
def fn73(x: str) -> str: ...
def fn73(x: int | str) -> int | str:
    return 1
def check73(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn73(a)
    fn73(b)
    fn73(c)
@overload
def fn74(x: str, y: int) -> str: ...
@overload
def fn74(x: bytes, y: int) -> bytes: ...
def fn74(x: str | bytes, y: int) -> str | bytes:
    return ""
def check74(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn74(a, 1)
    fn74(b, 2)
    fn74(c, 3)
@overload
def fn75(x: int, y: str, z: int) -> str: ...
@overload
def fn75(x: int, y: int, z: int) -> int: ...
def fn75(x: int, y: int | str, z: int) -> int | str:
    return 1
def check75(a: int | str, b: str | int, c: bytes | int) -> None:
    fn75(a, a, 1)
    fn75(b, b, 2)
    fn75(c, c, 3)
@overload
def fn76(x: int) -> int: ...
@overload
def fn76(x: str) -> str: ...
def fn76(x: int | str) -> int | str:
    return 1
def check76(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn76(a)
    fn76(b)
    fn76(c)
@overload
def fn77(x: str, y: int) -> str: ...
@overload
def fn77(x: bytes, y: int) -> bytes: ...
def fn77(x: str | bytes, y: int) -> str | bytes:
    return ""
def check77(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn77(a, 1)
    fn77(b, 2)
    fn77(c, 3)
@overload
def fn78(x: int, y: str, z: int) -> str: ...
@overload
def fn78(x: int, y: int, z: int) -> int: ...
def fn78(x: int, y: int | str, z: int) -> int | str:
    return 1
def check78(a: int | str, b: str | int, c: bytes | int) -> None:
    fn78(a, a, 1)
    fn78(b, b, 2)
    fn78(c, c, 3)
@overload
def fn79(x: int) -> int: ...
@overload
def fn79(x: str) -> str: ...
def fn79(x: int | str) -> int | str:
    return 1
def check79(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn79(a)
    fn79(b)
    fn79(c)
@overload
def fn80(x: str, y: int) -> str: ...
@overload
def fn80(x: bytes, y: int) -> bytes: ...
def fn80(x: str | bytes, y: int) -> str | bytes:
    return ""
def check80(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn80(a, 1)
    fn80(b, 2)
    fn80(c, 3)
@overload
def fn81(x: int, y: str, z: int) -> str: ...
@overload
def fn81(x: int, y: int, z: int) -> int: ...
def fn81(x: int, y: int | str, z: int) -> int | str:
    return 1
def check81(a: int | str, b: str | int, c: bytes | int) -> None:
    fn81(a, a, 1)
    fn81(b, b, 2)
    fn81(c, c, 3)
@overload
def fn82(x: int) -> int: ...
@overload
def fn82(x: str) -> str: ...
def fn82(x: int | str) -> int | str:
    return 1
def check82(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn82(a)
    fn82(b)
    fn82(c)
@overload
def fn83(x: str, y: int) -> str: ...
@overload
def fn83(x: bytes, y: int) -> bytes: ...
def fn83(x: str | bytes, y: int) -> str | bytes:
    return ""
def check83(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn83(a, 1)
    fn83(b, 2)
    fn83(c, 3)
@overload
def fn84(x: int, y: str, z: int) -> str: ...
@overload
def fn84(x: int, y: int, z: int) -> int: ...
def fn84(x: int, y: int | str, z: int) -> int | str:
    return 1
def check84(a: int | str, b: str | int, c: bytes | int) -> None:
    fn84(a, a, 1)
    fn84(b, b, 2)
    fn84(c, c, 3)
@overload
def fn85(x: int) -> int: ...
@overload
def fn85(x: str) -> str: ...
def fn85(x: int | str) -> int | str:
    return 1
def check85(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn85(a)
    fn85(b)
    fn85(c)
@overload
def fn86(x: str, y: int) -> str: ...
@overload
def fn86(x: bytes, y: int) -> bytes: ...
def fn86(x: str | bytes, y: int) -> str | bytes:
    return ""
def check86(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn86(a, 1)
    fn86(b, 2)
    fn86(c, 3)
@overload
def fn87(x: int, y: str, z: int) -> str: ...
@overload
def fn87(x: int, y: int, z: int) -> int: ...
def fn87(x: int, y: int | str, z: int) -> int | str:
    return 1
def check87(a: int | str, b: str | int, c: bytes | int) -> None:
    fn87(a, a, 1)
    fn87(b, b, 2)
    fn87(c, c, 3)
@overload
def fn88(x: int) -> int: ...
@overload
def fn88(x: str) -> str: ...
def fn88(x: int | str) -> int | str:
    return 1
def check88(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn88(a)
    fn88(b)
    fn88(c)
@overload
def fn89(x: str, y: int) -> str: ...
@overload
def fn89(x: bytes, y: int) -> bytes: ...
def fn89(x: str | bytes, y: int) -> str | bytes:
    return ""
def check89(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn89(a, 1)
    fn89(b, 2)
    fn89(c, 3)
@overload
def fn90(x: int, y: str, z: int) -> str: ...
@overload
def fn90(x: int, y: int, z: int) -> int: ...
def fn90(x: int, y: int | str, z: int) -> int | str:
    return 1
def check90(a: int | str, b: str | int, c: bytes | int) -> None:
    fn90(a, a, 1)
    fn90(b, b, 2)
    fn90(c, c, 3)
@overload
def fn91(x: int) -> int: ...
@overload
def fn91(x: str) -> str: ...
def fn91(x: int | str) -> int | str:
    return 1
def check91(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn91(a)
    fn91(b)
    fn91(c)
@overload
def fn92(x: str, y: int) -> str: ...
@overload
def fn92(x: bytes, y: int) -> bytes: ...
def fn92(x: str | bytes, y: int) -> str | bytes:
    return ""
def check92(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn92(a, 1)
    fn92(b, 2)
    fn92(c, 3)
@overload
def fn93(x: int, y: str, z: int) -> str: ...
@overload
def fn93(x: int, y: int, z: int) -> int: ...
def fn93(x: int, y: int | str, z: int) -> int | str:
    return 1
def check93(a: int | str, b: str | int, c: bytes | int) -> None:
    fn93(a, a, 1)
    fn93(b, b, 2)
    fn93(c, c, 3)
@overload
def fn94(x: int) -> int: ...
@overload
def fn94(x: str) -> str: ...
def fn94(x: int | str) -> int | str:
    return 1
def check94(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn94(a)
    fn94(b)
    fn94(c)
@overload
def fn95(x: str, y: int) -> str: ...
@overload
def fn95(x: bytes, y: int) -> bytes: ...
def fn95(x: str | bytes, y: int) -> str | bytes:
    return ""
def check95(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn95(a, 1)
    fn95(b, 2)
    fn95(c, 3)
@overload
def fn96(x: int, y: str, z: int) -> str: ...
@overload
def fn96(x: int, y: int, z: int) -> int: ...
def fn96(x: int, y: int | str, z: int) -> int | str:
    return 1
def check96(a: int | str, b: str | int, c: bytes | int) -> None:
    fn96(a, a, 1)
    fn96(b, b, 2)
    fn96(c, c, 3)
@overload
def fn97(x: int) -> int: ...
@overload
def fn97(x: str) -> str: ...
def fn97(x: int | str) -> int | str:
    return 1
def check97(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn97(a)
    fn97(b)
    fn97(c)
@overload
def fn98(x: str, y: int) -> str: ...
@overload
def fn98(x: bytes, y: int) -> bytes: ...
def fn98(x: str | bytes, y: int) -> str | bytes:
    return ""
def check98(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn98(a, 1)
    fn98(b, 2)
    fn98(c, 3)
@overload
def fn99(x: int, y: str, z: int) -> str: ...
@overload
def fn99(x: int, y: int, z: int) -> int: ...
def fn99(x: int, y: int | str, z: int) -> int | str:
    return 1
def check99(a: int | str, b: str | int, c: bytes | int) -> None:
    fn99(a, a, 1)
    fn99(b, b, 2)
    fn99(c, c, 3)
@overload
def fn100(x: int) -> int: ...
@overload
def fn100(x: str) -> str: ...
def fn100(x: int | str) -> int | str:
    return 1
def check100(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn100(a)
    fn100(b)
    fn100(c)
@overload
def fn101(x: str, y: int) -> str: ...
@overload
def fn101(x: bytes, y: int) -> bytes: ...
def fn101(x: str | bytes, y: int) -> str | bytes:
    return ""
def check101(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn101(a, 1)
    fn101(b, 2)
    fn101(c, 3)
@overload
def fn102(x: int, y: str, z: int) -> str: ...
@overload
def fn102(x: int, y: int, z: int) -> int: ...
def fn102(x: int, y: int | str, z: int) -> int | str:
    return 1
def check102(a: int | str, b: str | int, c: bytes | int) -> None:
    fn102(a, a, 1)
    fn102(b, b, 2)
    fn102(c, c, 3)
@overload
def fn103(x: int) -> int: ...
@overload
def fn103(x: str) -> str: ...
def fn103(x: int | str) -> int | str:
    return 1
def check103(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn103(a)
    fn103(b)
    fn103(c)
@overload
def fn104(x: str, y: int) -> str: ...
@overload
def fn104(x: bytes, y: int) -> bytes: ...
def fn104(x: str | bytes, y: int) -> str | bytes:
    return ""
def check104(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn104(a, 1)
    fn104(b, 2)
    fn104(c, 3)
@overload
def fn105(x: int, y: str, z: int) -> str: ...
@overload
def fn105(x: int, y: int, z: int) -> int: ...
def fn105(x: int, y: int | str, z: int) -> int | str:
    return 1
def check105(a: int | str, b: str | int, c: bytes | int) -> None:
    fn105(a, a, 1)
    fn105(b, b, 2)
    fn105(c, c, 3)
@overload
def fn106(x: int) -> int: ...
@overload
def fn106(x: str) -> str: ...
def fn106(x: int | str) -> int | str:
    return 1
def check106(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn106(a)
    fn106(b)
    fn106(c)
@overload
def fn107(x: str, y: int) -> str: ...
@overload
def fn107(x: bytes, y: int) -> bytes: ...
def fn107(x: str | bytes, y: int) -> str | bytes:
    return ""
def check107(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn107(a, 1)
    fn107(b, 2)
    fn107(c, 3)
@overload
def fn108(x: int, y: str, z: int) -> str: ...
@overload
def fn108(x: int, y: int, z: int) -> int: ...
def fn108(x: int, y: int | str, z: int) -> int | str:
    return 1
def check108(a: int | str, b: str | int, c: bytes | int) -> None:
    fn108(a, a, 1)
    fn108(b, b, 2)
    fn108(c, c, 3)
@overload
def fn109(x: int) -> int: ...
@overload
def fn109(x: str) -> str: ...
def fn109(x: int | str) -> int | str:
    return 1
def check109(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn109(a)
    fn109(b)
    fn109(c)
@overload
def fn110(x: str, y: int) -> str: ...
@overload
def fn110(x: bytes, y: int) -> bytes: ...
def fn110(x: str | bytes, y: int) -> str | bytes:
    return ""
def check110(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn110(a, 1)
    fn110(b, 2)
    fn110(c, 3)
@overload
def fn111(x: int, y: str, z: int) -> str: ...
@overload
def fn111(x: int, y: int, z: int) -> int: ...
def fn111(x: int, y: int | str, z: int) -> int | str:
    return 1
def check111(a: int | str, b: str | int, c: bytes | int) -> None:
    fn111(a, a, 1)
    fn111(b, b, 2)
    fn111(c, c, 3)
@overload
def fn112(x: int) -> int: ...
@overload
def fn112(x: str) -> str: ...
def fn112(x: int | str) -> int | str:
    return 1
def check112(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn112(a)
    fn112(b)
    fn112(c)
@overload
def fn113(x: str, y: int) -> str: ...
@overload
def fn113(x: bytes, y: int) -> bytes: ...
def fn113(x: str | bytes, y: int) -> str | bytes:
    return ""
def check113(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn113(a, 1)
    fn113(b, 2)
    fn113(c, 3)
@overload
def fn114(x: int, y: str, z: int) -> str: ...
@overload
def fn114(x: int, y: int, z: int) -> int: ...
def fn114(x: int, y: int | str, z: int) -> int | str:
    return 1
def check114(a: int | str, b: str | int, c: bytes | int) -> None:
    fn114(a, a, 1)
    fn114(b, b, 2)
    fn114(c, c, 3)
@overload
def fn115(x: int) -> int: ...
@overload
def fn115(x: str) -> str: ...
def fn115(x: int | str) -> int | str:
    return 1
def check115(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn115(a)
    fn115(b)
    fn115(c)
@overload
def fn116(x: str, y: int) -> str: ...
@overload
def fn116(x: bytes, y: int) -> bytes: ...
def fn116(x: str | bytes, y: int) -> str | bytes:
    return ""
def check116(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn116(a, 1)
    fn116(b, 2)
    fn116(c, 3)
@overload
def fn117(x: int, y: str, z: int) -> str: ...
@overload
def fn117(x: int, y: int, z: int) -> int: ...
def fn117(x: int, y: int | str, z: int) -> int | str:
    return 1
def check117(a: int | str, b: str | int, c: bytes | int) -> None:
    fn117(a, a, 1)
    fn117(b, b, 2)
    fn117(c, c, 3)
@overload
def fn118(x: int) -> int: ...
@overload
def fn118(x: str) -> str: ...
def fn118(x: int | str) -> int | str:
    return 1
def check118(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn118(a)
    fn118(b)
    fn118(c)
@overload
def fn119(x: str, y: int) -> str: ...
@overload
def fn119(x: bytes, y: int) -> bytes: ...
def fn119(x: str | bytes, y: int) -> str | bytes:
    return ""
def check119(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn119(a, 1)
    fn119(b, 2)
    fn119(c, 3)
@overload
def fn120(x: int, y: str, z: int) -> str: ...
@overload
def fn120(x: int, y: int, z: int) -> int: ...
def fn120(x: int, y: int | str, z: int) -> int | str:
    return 1
def check120(a: int | str, b: str | int, c: bytes | int) -> None:
    fn120(a, a, 1)
    fn120(b, b, 2)
    fn120(c, c, 3)
@overload
def fn121(x: int) -> int: ...
@overload
def fn121(x: str) -> str: ...
def fn121(x: int | str) -> int | str:
    return 1
def check121(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn121(a)
    fn121(b)
    fn121(c)
@overload
def fn122(x: str, y: int) -> str: ...
@overload
def fn122(x: bytes, y: int) -> bytes: ...
def fn122(x: str | bytes, y: int) -> str | bytes:
    return ""
def check122(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn122(a, 1)
    fn122(b, 2)
    fn122(c, 3)
@overload
def fn123(x: int, y: str, z: int) -> str: ...
@overload
def fn123(x: int, y: int, z: int) -> int: ...
def fn123(x: int, y: int | str, z: int) -> int | str:
    return 1
def check123(a: int | str, b: str | int, c: bytes | int) -> None:
    fn123(a, a, 1)
    fn123(b, b, 2)
    fn123(c, c, 3)
@overload
def fn124(x: int) -> int: ...
@overload
def fn124(x: str) -> str: ...
def fn124(x: int | str) -> int | str:
    return 1
def check124(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn124(a)
    fn124(b)
    fn124(c)
@overload
def fn125(x: str, y: int) -> str: ...
@overload
def fn125(x: bytes, y: int) -> bytes: ...
def fn125(x: str | bytes, y: int) -> str | bytes:
    return ""
def check125(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn125(a, 1)
    fn125(b, 2)
    fn125(c, 3)
@overload
def fn126(x: int, y: str, z: int) -> str: ...
@overload
def fn126(x: int, y: int, z: int) -> int: ...
def fn126(x: int, y: int | str, z: int) -> int | str:
    return 1
def check126(a: int | str, b: str | int, c: bytes | int) -> None:
    fn126(a, a, 1)
    fn126(b, b, 2)
    fn126(c, c, 3)
@overload
def fn127(x: int) -> int: ...
@overload
def fn127(x: str) -> str: ...
def fn127(x: int | str) -> int | str:
    return 1
def check127(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn127(a)
    fn127(b)
    fn127(c)
@overload
def fn128(x: str, y: int) -> str: ...
@overload
def fn128(x: bytes, y: int) -> bytes: ...
def fn128(x: str | bytes, y: int) -> str | bytes:
    return ""
def check128(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn128(a, 1)
    fn128(b, 2)
    fn128(c, 3)
@overload
def fn129(x: int, y: str, z: int) -> str: ...
@overload
def fn129(x: int, y: int, z: int) -> int: ...
def fn129(x: int, y: int | str, z: int) -> int | str:
    return 1
def check129(a: int | str, b: str | int, c: bytes | int) -> None:
    fn129(a, a, 1)
    fn129(b, b, 2)
    fn129(c, c, 3)
@overload
def fn130(x: int) -> int: ...
@overload
def fn130(x: str) -> str: ...
def fn130(x: int | str) -> int | str:
    return 1
def check130(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn130(a)
    fn130(b)
    fn130(c)
@overload
def fn131(x: str, y: int) -> str: ...
@overload
def fn131(x: bytes, y: int) -> bytes: ...
def fn131(x: str | bytes, y: int) -> str | bytes:
    return ""
def check131(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn131(a, 1)
    fn131(b, 2)
    fn131(c, 3)
@overload
def fn132(x: int, y: str, z: int) -> str: ...
@overload
def fn132(x: int, y: int, z: int) -> int: ...
def fn132(x: int, y: int | str, z: int) -> int | str:
    return 1
def check132(a: int | str, b: str | int, c: bytes | int) -> None:
    fn132(a, a, 1)
    fn132(b, b, 2)
    fn132(c, c, 3)
@overload
def fn133(x: int) -> int: ...
@overload
def fn133(x: str) -> str: ...
def fn133(x: int | str) -> int | str:
    return 1
def check133(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn133(a)
    fn133(b)
    fn133(c)
@overload
def fn134(x: str, y: int) -> str: ...
@overload
def fn134(x: bytes, y: int) -> bytes: ...
def fn134(x: str | bytes, y: int) -> str | bytes:
    return ""
def check134(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn134(a, 1)
    fn134(b, 2)
    fn134(c, 3)
@overload
def fn135(x: int, y: str, z: int) -> str: ...
@overload
def fn135(x: int, y: int, z: int) -> int: ...
def fn135(x: int, y: int | str, z: int) -> int | str:
    return 1
def check135(a: int | str, b: str | int, c: bytes | int) -> None:
    fn135(a, a, 1)
    fn135(b, b, 2)
    fn135(c, c, 3)
@overload
def fn136(x: int) -> int: ...
@overload
def fn136(x: str) -> str: ...
def fn136(x: int | str) -> int | str:
    return 1
def check136(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn136(a)
    fn136(b)
    fn136(c)
@overload
def fn137(x: str, y: int) -> str: ...
@overload
def fn137(x: bytes, y: int) -> bytes: ...
def fn137(x: str | bytes, y: int) -> str | bytes:
    return ""
def check137(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn137(a, 1)
    fn137(b, 2)
    fn137(c, 3)
@overload
def fn138(x: int, y: str, z: int) -> str: ...
@overload
def fn138(x: int, y: int, z: int) -> int: ...
def fn138(x: int, y: int | str, z: int) -> int | str:
    return 1
def check138(a: int | str, b: str | int, c: bytes | int) -> None:
    fn138(a, a, 1)
    fn138(b, b, 2)
    fn138(c, c, 3)
@overload
def fn139(x: int) -> int: ...
@overload
def fn139(x: str) -> str: ...
def fn139(x: int | str) -> int | str:
    return 1
def check139(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn139(a)
    fn139(b)
    fn139(c)
@overload
def fn140(x: str, y: int) -> str: ...
@overload
def fn140(x: bytes, y: int) -> bytes: ...
def fn140(x: str | bytes, y: int) -> str | bytes:
    return ""
def check140(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn140(a, 1)
    fn140(b, 2)
    fn140(c, 3)
@overload
def fn141(x: int, y: str, z: int) -> str: ...
@overload
def fn141(x: int, y: int, z: int) -> int: ...
def fn141(x: int, y: int | str, z: int) -> int | str:
    return 1
def check141(a: int | str, b: str | int, c: bytes | int) -> None:
    fn141(a, a, 1)
    fn141(b, b, 2)
    fn141(c, c, 3)
@overload
def fn142(x: int) -> int: ...
@overload
def fn142(x: str) -> str: ...
def fn142(x: int | str) -> int | str:
    return 1
def check142(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn142(a)
    fn142(b)
    fn142(c)
@overload
def fn143(x: str, y: int) -> str: ...
@overload
def fn143(x: bytes, y: int) -> bytes: ...
def fn143(x: str | bytes, y: int) -> str | bytes:
    return ""
def check143(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn143(a, 1)
    fn143(b, 2)
    fn143(c, 3)
@overload
def fn144(x: int, y: str, z: int) -> str: ...
@overload
def fn144(x: int, y: int, z: int) -> int: ...
def fn144(x: int, y: int | str, z: int) -> int | str:
    return 1
def check144(a: int | str, b: str | int, c: bytes | int) -> None:
    fn144(a, a, 1)
    fn144(b, b, 2)
    fn144(c, c, 3)
@overload
def fn145(x: int) -> int: ...
@overload
def fn145(x: str) -> str: ...
def fn145(x: int | str) -> int | str:
    return 1
def check145(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn145(a)
    fn145(b)
    fn145(c)
@overload
def fn146(x: str, y: int) -> str: ...
@overload
def fn146(x: bytes, y: int) -> bytes: ...
def fn146(x: str | bytes, y: int) -> str | bytes:
    return ""
def check146(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn146(a, 1)
    fn146(b, 2)
    fn146(c, 3)
@overload
def fn147(x: int, y: str, z: int) -> str: ...
@overload
def fn147(x: int, y: int, z: int) -> int: ...
def fn147(x: int, y: int | str, z: int) -> int | str:
    return 1
def check147(a: int | str, b: str | int, c: bytes | int) -> None:
    fn147(a, a, 1)
    fn147(b, b, 2)
    fn147(c, c, 3)
@overload
def fn148(x: int) -> int: ...
@overload
def fn148(x: str) -> str: ...
def fn148(x: int | str) -> int | str:
    return 1
def check148(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn148(a)
    fn148(b)
    fn148(c)
@overload
def fn149(x: str, y: int) -> str: ...
@overload
def fn149(x: bytes, y: int) -> bytes: ...
def fn149(x: str | bytes, y: int) -> str | bytes:
    return ""
def check149(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn149(a, 1)
    fn149(b, 2)
    fn149(c, 3)
@overload
def fn150(x: int, y: str, z: int) -> str: ...
@overload
def fn150(x: int, y: int, z: int) -> int: ...
def fn150(x: int, y: int | str, z: int) -> int | str:
    return 1
def check150(a: int | str, b: str | int, c: bytes | int) -> None:
    fn150(a, a, 1)
    fn150(b, b, 2)
    fn150(c, c, 3)
@overload
def fn151(x: int) -> int: ...
@overload
def fn151(x: str) -> str: ...
def fn151(x: int | str) -> int | str:
    return 1
def check151(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn151(a)
    fn151(b)
    fn151(c)
@overload
def fn152(x: str, y: int) -> str: ...
@overload
def fn152(x: bytes, y: int) -> bytes: ...
def fn152(x: str | bytes, y: int) -> str | bytes:
    return ""
def check152(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn152(a, 1)
    fn152(b, 2)
    fn152(c, 3)
@overload
def fn153(x: int, y: str, z: int) -> str: ...
@overload
def fn153(x: int, y: int, z: int) -> int: ...
def fn153(x: int, y: int | str, z: int) -> int | str:
    return 1
def check153(a: int | str, b: str | int, c: bytes | int) -> None:
    fn153(a, a, 1)
    fn153(b, b, 2)
    fn153(c, c, 3)
@overload
def fn154(x: int) -> int: ...
@overload
def fn154(x: str) -> str: ...
def fn154(x: int | str) -> int | str:
    return 1
def check154(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn154(a)
    fn154(b)
    fn154(c)
@overload
def fn155(x: str, y: int) -> str: ...
@overload
def fn155(x: bytes, y: int) -> bytes: ...
def fn155(x: str | bytes, y: int) -> str | bytes:
    return ""
def check155(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn155(a, 1)
    fn155(b, 2)
    fn155(c, 3)
@overload
def fn156(x: int, y: str, z: int) -> str: ...
@overload
def fn156(x: int, y: int, z: int) -> int: ...
def fn156(x: int, y: int | str, z: int) -> int | str:
    return 1
def check156(a: int | str, b: str | int, c: bytes | int) -> None:
    fn156(a, a, 1)
    fn156(b, b, 2)
    fn156(c, c, 3)
@overload
def fn157(x: int) -> int: ...
@overload
def fn157(x: str) -> str: ...
def fn157(x: int | str) -> int | str:
    return 1
def check157(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn157(a)
    fn157(b)
    fn157(c)
@overload
def fn158(x: str, y: int) -> str: ...
@overload
def fn158(x: bytes, y: int) -> bytes: ...
def fn158(x: str | bytes, y: int) -> str | bytes:
    return ""
def check158(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn158(a, 1)
    fn158(b, 2)
    fn158(c, 3)
@overload
def fn159(x: int, y: str, z: int) -> str: ...
@overload
def fn159(x: int, y: int, z: int) -> int: ...
def fn159(x: int, y: int | str, z: int) -> int | str:
    return 1
def check159(a: int | str, b: str | int, c: bytes | int) -> None:
    fn159(a, a, 1)
    fn159(b, b, 2)
    fn159(c, c, 3)
@overload
def fn160(x: int) -> int: ...
@overload
def fn160(x: str) -> str: ...
def fn160(x: int | str) -> int | str:
    return 1
def check160(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn160(a)
    fn160(b)
    fn160(c)
@overload
def fn161(x: str, y: int) -> str: ...
@overload
def fn161(x: bytes, y: int) -> bytes: ...
def fn161(x: str | bytes, y: int) -> str | bytes:
    return ""
def check161(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn161(a, 1)
    fn161(b, 2)
    fn161(c, 3)
@overload
def fn162(x: int, y: str, z: int) -> str: ...
@overload
def fn162(x: int, y: int, z: int) -> int: ...
def fn162(x: int, y: int | str, z: int) -> int | str:
    return 1
def check162(a: int | str, b: str | int, c: bytes | int) -> None:
    fn162(a, a, 1)
    fn162(b, b, 2)
    fn162(c, c, 3)
@overload
def fn163(x: int) -> int: ...
@overload
def fn163(x: str) -> str: ...
def fn163(x: int | str) -> int | str:
    return 1
def check163(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn163(a)
    fn163(b)
    fn163(c)
@overload
def fn164(x: str, y: int) -> str: ...
@overload
def fn164(x: bytes, y: int) -> bytes: ...
def fn164(x: str | bytes, y: int) -> str | bytes:
    return ""
def check164(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn164(a, 1)
    fn164(b, 2)
    fn164(c, 3)
@overload
def fn165(x: int, y: str, z: int) -> str: ...
@overload
def fn165(x: int, y: int, z: int) -> int: ...
def fn165(x: int, y: int | str, z: int) -> int | str:
    return 1
def check165(a: int | str, b: str | int, c: bytes | int) -> None:
    fn165(a, a, 1)
    fn165(b, b, 2)
    fn165(c, c, 3)
@overload
def fn166(x: int) -> int: ...
@overload
def fn166(x: str) -> str: ...
def fn166(x: int | str) -> int | str:
    return 1
def check166(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn166(a)
    fn166(b)
    fn166(c)
@overload
def fn167(x: str, y: int) -> str: ...
@overload
def fn167(x: bytes, y: int) -> bytes: ...
def fn167(x: str | bytes, y: int) -> str | bytes:
    return ""
def check167(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn167(a, 1)
    fn167(b, 2)
    fn167(c, 3)
@overload
def fn168(x: int, y: str, z: int) -> str: ...
@overload
def fn168(x: int, y: int, z: int) -> int: ...
def fn168(x: int, y: int | str, z: int) -> int | str:
    return 1
def check168(a: int | str, b: str | int, c: bytes | int) -> None:
    fn168(a, a, 1)
    fn168(b, b, 2)
    fn168(c, c, 3)
@overload
def fn169(x: int) -> int: ...
@overload
def fn169(x: str) -> str: ...
def fn169(x: int | str) -> int | str:
    return 1
def check169(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn169(a)
    fn169(b)
    fn169(c)
@overload
def fn170(x: str, y: int) -> str: ...
@overload
def fn170(x: bytes, y: int) -> bytes: ...
def fn170(x: str | bytes, y: int) -> str | bytes:
    return ""
def check170(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn170(a, 1)
    fn170(b, 2)
    fn170(c, 3)
@overload
def fn171(x: int, y: str, z: int) -> str: ...
@overload
def fn171(x: int, y: int, z: int) -> int: ...
def fn171(x: int, y: int | str, z: int) -> int | str:
    return 1
def check171(a: int | str, b: str | int, c: bytes | int) -> None:
    fn171(a, a, 1)
    fn171(b, b, 2)
    fn171(c, c, 3)
@overload
def fn172(x: int) -> int: ...
@overload
def fn172(x: str) -> str: ...
def fn172(x: int | str) -> int | str:
    return 1
def check172(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn172(a)
    fn172(b)
    fn172(c)
@overload
def fn173(x: str, y: int) -> str: ...
@overload
def fn173(x: bytes, y: int) -> bytes: ...
def fn173(x: str | bytes, y: int) -> str | bytes:
    return ""
def check173(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn173(a, 1)
    fn173(b, 2)
    fn173(c, 3)
@overload
def fn174(x: int, y: str, z: int) -> str: ...
@overload
def fn174(x: int, y: int, z: int) -> int: ...
def fn174(x: int, y: int | str, z: int) -> int | str:
    return 1
def check174(a: int | str, b: str | int, c: bytes | int) -> None:
    fn174(a, a, 1)
    fn174(b, b, 2)
    fn174(c, c, 3)
@overload
def fn175(x: int) -> int: ...
@overload
def fn175(x: str) -> str: ...
def fn175(x: int | str) -> int | str:
    return 1
def check175(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn175(a)
    fn175(b)
    fn175(c)
@overload
def fn176(x: str, y: int) -> str: ...
@overload
def fn176(x: bytes, y: int) -> bytes: ...
def fn176(x: str | bytes, y: int) -> str | bytes:
    return ""
def check176(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn176(a, 1)
    fn176(b, 2)
    fn176(c, 3)
@overload
def fn177(x: int, y: str, z: int) -> str: ...
@overload
def fn177(x: int, y: int, z: int) -> int: ...
def fn177(x: int, y: int | str, z: int) -> int | str:
    return 1
def check177(a: int | str, b: str | int, c: bytes | int) -> None:
    fn177(a, a, 1)
    fn177(b, b, 2)
    fn177(c, c, 3)
@overload
def fn178(x: int) -> int: ...
@overload
def fn178(x: str) -> str: ...
def fn178(x: int | str) -> int | str:
    return 1
def check178(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn178(a)
    fn178(b)
    fn178(c)
@overload
def fn179(x: str, y: int) -> str: ...
@overload
def fn179(x: bytes, y: int) -> bytes: ...
def fn179(x: str | bytes, y: int) -> str | bytes:
    return ""
def check179(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn179(a, 1)
    fn179(b, 2)
    fn179(c, 3)
@overload
def fn180(x: int, y: str, z: int) -> str: ...
@overload
def fn180(x: int, y: int, z: int) -> int: ...
def fn180(x: int, y: int | str, z: int) -> int | str:
    return 1
def check180(a: int | str, b: str | int, c: bytes | int) -> None:
    fn180(a, a, 1)
    fn180(b, b, 2)
    fn180(c, c, 3)
@overload
def fn181(x: int) -> int: ...
@overload
def fn181(x: str) -> str: ...
def fn181(x: int | str) -> int | str:
    return 1
def check181(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn181(a)
    fn181(b)
    fn181(c)
@overload
def fn182(x: str, y: int) -> str: ...
@overload
def fn182(x: bytes, y: int) -> bytes: ...
def fn182(x: str | bytes, y: int) -> str | bytes:
    return ""
def check182(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn182(a, 1)
    fn182(b, 2)
    fn182(c, 3)
@overload
def fn183(x: int, y: str, z: int) -> str: ...
@overload
def fn183(x: int, y: int, z: int) -> int: ...
def fn183(x: int, y: int | str, z: int) -> int | str:
    return 1
def check183(a: int | str, b: str | int, c: bytes | int) -> None:
    fn183(a, a, 1)
    fn183(b, b, 2)
    fn183(c, c, 3)
@overload
def fn184(x: int) -> int: ...
@overload
def fn184(x: str) -> str: ...
def fn184(x: int | str) -> int | str:
    return 1
def check184(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn184(a)
    fn184(b)
    fn184(c)
@overload
def fn185(x: str, y: int) -> str: ...
@overload
def fn185(x: bytes, y: int) -> bytes: ...
def fn185(x: str | bytes, y: int) -> str | bytes:
    return ""
def check185(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn185(a, 1)
    fn185(b, 2)
    fn185(c, 3)
@overload
def fn186(x: int, y: str, z: int) -> str: ...
@overload
def fn186(x: int, y: int, z: int) -> int: ...
def fn186(x: int, y: int | str, z: int) -> int | str:
    return 1
def check186(a: int | str, b: str | int, c: bytes | int) -> None:
    fn186(a, a, 1)
    fn186(b, b, 2)
    fn186(c, c, 3)
@overload
def fn187(x: int) -> int: ...
@overload
def fn187(x: str) -> str: ...
def fn187(x: int | str) -> int | str:
    return 1
def check187(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn187(a)
    fn187(b)
    fn187(c)
@overload
def fn188(x: str, y: int) -> str: ...
@overload
def fn188(x: bytes, y: int) -> bytes: ...
def fn188(x: str | bytes, y: int) -> str | bytes:
    return ""
def check188(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn188(a, 1)
    fn188(b, 2)
    fn188(c, 3)
@overload
def fn189(x: int, y: str, z: int) -> str: ...
@overload
def fn189(x: int, y: int, z: int) -> int: ...
def fn189(x: int, y: int | str, z: int) -> int | str:
    return 1
def check189(a: int | str, b: str | int, c: bytes | int) -> None:
    fn189(a, a, 1)
    fn189(b, b, 2)
    fn189(c, c, 3)
@overload
def fn190(x: int) -> int: ...
@overload
def fn190(x: str) -> str: ...
def fn190(x: int | str) -> int | str:
    return 1
def check190(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn190(a)
    fn190(b)
    fn190(c)
@overload
def fn191(x: str, y: int) -> str: ...
@overload
def fn191(x: bytes, y: int) -> bytes: ...
def fn191(x: str | bytes, y: int) -> str | bytes:
    return ""
def check191(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn191(a, 1)
    fn191(b, 2)
    fn191(c, 3)
@overload
def fn192(x: int, y: str, z: int) -> str: ...
@overload
def fn192(x: int, y: int, z: int) -> int: ...
def fn192(x: int, y: int | str, z: int) -> int | str:
    return 1
def check192(a: int | str, b: str | int, c: bytes | int) -> None:
    fn192(a, a, 1)
    fn192(b, b, 2)
    fn192(c, c, 3)
@overload
def fn193(x: int) -> int: ...
@overload
def fn193(x: str) -> str: ...
def fn193(x: int | str) -> int | str:
    return 1
def check193(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn193(a)
    fn193(b)
    fn193(c)
@overload
def fn194(x: str, y: int) -> str: ...
@overload
def fn194(x: bytes, y: int) -> bytes: ...
def fn194(x: str | bytes, y: int) -> str | bytes:
    return ""
def check194(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn194(a, 1)
    fn194(b, 2)
    fn194(c, 3)
@overload
def fn195(x: int, y: str, z: int) -> str: ...
@overload
def fn195(x: int, y: int, z: int) -> int: ...
def fn195(x: int, y: int | str, z: int) -> int | str:
    return 1
def check195(a: int | str, b: str | int, c: bytes | int) -> None:
    fn195(a, a, 1)
    fn195(b, b, 2)
    fn195(c, c, 3)
@overload
def fn196(x: int) -> int: ...
@overload
def fn196(x: str) -> str: ...
def fn196(x: int | str) -> int | str:
    return 1
def check196(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn196(a)
    fn196(b)
    fn196(c)
@overload
def fn197(x: str, y: int) -> str: ...
@overload
def fn197(x: bytes, y: int) -> bytes: ...
def fn197(x: str | bytes, y: int) -> str | bytes:
    return ""
def check197(a: str | int, b: Union[bytes, float], c: int | bytes) -> None:
    fn197(a, 1)
    fn197(b, 2)
    fn197(c, 3)
@overload
def fn198(x: int, y: str, z: int) -> str: ...
@overload
def fn198(x: int, y: int, z: int) -> int: ...
def fn198(x: int, y: int | str, z: int) -> int | str:
    return 1
def check198(a: int | str, b: str | int, c: bytes | int) -> None:
    fn198(a, a, 1)
    fn198(b, b, 2)
    fn198(c, c, 3)
@overload
def fn199(x: int) -> int: ...
@overload
def fn199(x: str) -> str: ...
def fn199(x: int | str) -> int | str:
    return 1
def check199(a: Union[int, str, bytes], b: str | bytes, c: Union[bytes, int]) -> None:
    fn199(a)
    fn199(b)
    fn199(c)
