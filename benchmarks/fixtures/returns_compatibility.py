# type: disabled[returns_compatibility_2]
# Benchmark stress fixture for `returns_compatibility` (return type mismatch).
# The sibling rule returns_compatibility_2 fires in lock-step on the same
# constructs by design; the block directive above isolates the target rule so
# the fixture stays single-rule. It is an inert comment for other checkers.


def fn0_a() -> str:
    return 0


def fn0_b(flag: bool) -> int:
    if flag:
        return "s0"
    return 2.5


def fn0_c() -> None:
    return 0


class Box0:
    def get(self) -> list[int]:
        return "items0"

    def size(self) -> dict[str, int]:
        return (1, 0)


def fn1_a() -> str:
    return 1


def fn1_b(flag: bool) -> int:
    if flag:
        return "s1"
    return 2.5


def fn1_c() -> None:
    return 1


class Box1:
    def get(self) -> list[int]:
        return "items1"

    def size(self) -> dict[str, int]:
        return (1, 1)


def fn2_a() -> str:
    return 2


def fn2_b(flag: bool) -> int:
    if flag:
        return "s2"
    return 2.5


def fn2_c() -> None:
    return 2


class Box2:
    def get(self) -> list[int]:
        return "items2"

    def size(self) -> dict[str, int]:
        return (1, 2)


def fn3_a() -> str:
    return 3


def fn3_b(flag: bool) -> int:
    if flag:
        return "s3"
    return 2.5


def fn3_c() -> None:
    return 3


class Box3:
    def get(self) -> list[int]:
        return "items3"

    def size(self) -> dict[str, int]:
        return (1, 3)


def fn4_a() -> str:
    return 4


def fn4_b(flag: bool) -> int:
    if flag:
        return "s4"
    return 2.5


def fn4_c() -> None:
    return 4


class Box4:
    def get(self) -> list[int]:
        return "items4"

    def size(self) -> dict[str, int]:
        return (1, 4)


def fn5_a() -> str:
    return 5


def fn5_b(flag: bool) -> int:
    if flag:
        return "s5"
    return 2.5


def fn5_c() -> None:
    return 5


class Box5:
    def get(self) -> list[int]:
        return "items5"

    def size(self) -> dict[str, int]:
        return (1, 5)


def fn6_a() -> str:
    return 6


def fn6_b(flag: bool) -> int:
    if flag:
        return "s6"
    return 2.5


def fn6_c() -> None:
    return 6


class Box6:
    def get(self) -> list[int]:
        return "items6"

    def size(self) -> dict[str, int]:
        return (1, 6)


def fn7_a() -> str:
    return 7


def fn7_b(flag: bool) -> int:
    if flag:
        return "s7"
    return 2.5


def fn7_c() -> None:
    return 7


class Box7:
    def get(self) -> list[int]:
        return "items7"

    def size(self) -> dict[str, int]:
        return (1, 7)


def fn8_a() -> str:
    return 8


def fn8_b(flag: bool) -> int:
    if flag:
        return "s8"
    return 2.5


def fn8_c() -> None:
    return 8


class Box8:
    def get(self) -> list[int]:
        return "items8"

    def size(self) -> dict[str, int]:
        return (1, 8)


def fn9_a() -> str:
    return 9


def fn9_b(flag: bool) -> int:
    if flag:
        return "s9"
    return 2.5


def fn9_c() -> None:
    return 9


class Box9:
    def get(self) -> list[int]:
        return "items9"

    def size(self) -> dict[str, int]:
        return (1, 9)


def fn10_a() -> str:
    return 10


def fn10_b(flag: bool) -> int:
    if flag:
        return "s10"
    return 2.5


def fn10_c() -> None:
    return 10


class Box10:
    def get(self) -> list[int]:
        return "items10"

    def size(self) -> dict[str, int]:
        return (1, 10)


def fn11_a() -> str:
    return 11


def fn11_b(flag: bool) -> int:
    if flag:
        return "s11"
    return 2.5


def fn11_c() -> None:
    return 11


class Box11:
    def get(self) -> list[int]:
        return "items11"

    def size(self) -> dict[str, int]:
        return (1, 11)


def fn12_a() -> str:
    return 12


def fn12_b(flag: bool) -> int:
    if flag:
        return "s12"
    return 2.5


def fn12_c() -> None:
    return 12


class Box12:
    def get(self) -> list[int]:
        return "items12"

    def size(self) -> dict[str, int]:
        return (1, 12)


def fn13_a() -> str:
    return 13


def fn13_b(flag: bool) -> int:
    if flag:
        return "s13"
    return 2.5


def fn13_c() -> None:
    return 13


class Box13:
    def get(self) -> list[int]:
        return "items13"

    def size(self) -> dict[str, int]:
        return (1, 13)


def fn14_a() -> str:
    return 14


def fn14_b(flag: bool) -> int:
    if flag:
        return "s14"
    return 2.5


def fn14_c() -> None:
    return 14


class Box14:
    def get(self) -> list[int]:
        return "items14"

    def size(self) -> dict[str, int]:
        return (1, 14)


def fn15_a() -> str:
    return 15


def fn15_b(flag: bool) -> int:
    if flag:
        return "s15"
    return 2.5


def fn15_c() -> None:
    return 15


class Box15:
    def get(self) -> list[int]:
        return "items15"

    def size(self) -> dict[str, int]:
        return (1, 15)


def fn16_a() -> str:
    return 16


def fn16_b(flag: bool) -> int:
    if flag:
        return "s16"
    return 2.5


def fn16_c() -> None:
    return 16


class Box16:
    def get(self) -> list[int]:
        return "items16"

    def size(self) -> dict[str, int]:
        return (1, 16)


def fn17_a() -> str:
    return 17


def fn17_b(flag: bool) -> int:
    if flag:
        return "s17"
    return 2.5


def fn17_c() -> None:
    return 17


class Box17:
    def get(self) -> list[int]:
        return "items17"

    def size(self) -> dict[str, int]:
        return (1, 17)


def fn18_a() -> str:
    return 18


def fn18_b(flag: bool) -> int:
    if flag:
        return "s18"
    return 2.5


def fn18_c() -> None:
    return 18


class Box18:
    def get(self) -> list[int]:
        return "items18"

    def size(self) -> dict[str, int]:
        return (1, 18)


def fn19_a() -> str:
    return 19


def fn19_b(flag: bool) -> int:
    if flag:
        return "s19"
    return 2.5


def fn19_c() -> None:
    return 19


class Box19:
    def get(self) -> list[int]:
        return "items19"

    def size(self) -> dict[str, int]:
        return (1, 19)


def fn20_a() -> str:
    return 20


def fn20_b(flag: bool) -> int:
    if flag:
        return "s20"
    return 2.5


def fn20_c() -> None:
    return 20


class Box20:
    def get(self) -> list[int]:
        return "items20"

    def size(self) -> dict[str, int]:
        return (1, 20)


def fn21_a() -> str:
    return 21


def fn21_b(flag: bool) -> int:
    if flag:
        return "s21"
    return 2.5


def fn21_c() -> None:
    return 21


class Box21:
    def get(self) -> list[int]:
        return "items21"

    def size(self) -> dict[str, int]:
        return (1, 21)


def fn22_a() -> str:
    return 22


def fn22_b(flag: bool) -> int:
    if flag:
        return "s22"
    return 2.5


def fn22_c() -> None:
    return 22


class Box22:
    def get(self) -> list[int]:
        return "items22"

    def size(self) -> dict[str, int]:
        return (1, 22)


def fn23_a() -> str:
    return 23


def fn23_b(flag: bool) -> int:
    if flag:
        return "s23"
    return 2.5


def fn23_c() -> None:
    return 23


class Box23:
    def get(self) -> list[int]:
        return "items23"

    def size(self) -> dict[str, int]:
        return (1, 23)


def fn24_a() -> str:
    return 24


def fn24_b(flag: bool) -> int:
    if flag:
        return "s24"
    return 2.5


def fn24_c() -> None:
    return 24


class Box24:
    def get(self) -> list[int]:
        return "items24"

    def size(self) -> dict[str, int]:
        return (1, 24)


def fn25_a() -> str:
    return 25


def fn25_b(flag: bool) -> int:
    if flag:
        return "s25"
    return 2.5


def fn25_c() -> None:
    return 25


class Box25:
    def get(self) -> list[int]:
        return "items25"

    def size(self) -> dict[str, int]:
        return (1, 25)


def fn26_a() -> str:
    return 26


def fn26_b(flag: bool) -> int:
    if flag:
        return "s26"
    return 2.5


def fn26_c() -> None:
    return 26


class Box26:
    def get(self) -> list[int]:
        return "items26"

    def size(self) -> dict[str, int]:
        return (1, 26)


def fn27_a() -> str:
    return 27


def fn27_b(flag: bool) -> int:
    if flag:
        return "s27"
    return 2.5


def fn27_c() -> None:
    return 27


class Box27:
    def get(self) -> list[int]:
        return "items27"

    def size(self) -> dict[str, int]:
        return (1, 27)


def fn28_a() -> str:
    return 28


def fn28_b(flag: bool) -> int:
    if flag:
        return "s28"
    return 2.5


def fn28_c() -> None:
    return 28


class Box28:
    def get(self) -> list[int]:
        return "items28"

    def size(self) -> dict[str, int]:
        return (1, 28)


def fn29_a() -> str:
    return 29


def fn29_b(flag: bool) -> int:
    if flag:
        return "s29"
    return 2.5


def fn29_c() -> None:
    return 29


class Box29:
    def get(self) -> list[int]:
        return "items29"

    def size(self) -> dict[str, int]:
        return (1, 29)


def fn30_a() -> str:
    return 30


def fn30_b(flag: bool) -> int:
    if flag:
        return "s30"
    return 2.5


def fn30_c() -> None:
    return 30


class Box30:
    def get(self) -> list[int]:
        return "items30"

    def size(self) -> dict[str, int]:
        return (1, 30)


def fn31_a() -> str:
    return 31


def fn31_b(flag: bool) -> int:
    if flag:
        return "s31"
    return 2.5


def fn31_c() -> None:
    return 31


class Box31:
    def get(self) -> list[int]:
        return "items31"

    def size(self) -> dict[str, int]:
        return (1, 31)


def fn32_a() -> str:
    return 32


def fn32_b(flag: bool) -> int:
    if flag:
        return "s32"
    return 2.5


def fn32_c() -> None:
    return 32


class Box32:
    def get(self) -> list[int]:
        return "items32"

    def size(self) -> dict[str, int]:
        return (1, 32)


def fn33_a() -> str:
    return 33


def fn33_b(flag: bool) -> int:
    if flag:
        return "s33"
    return 2.5


def fn33_c() -> None:
    return 33


class Box33:
    def get(self) -> list[int]:
        return "items33"

    def size(self) -> dict[str, int]:
        return (1, 33)


def fn34_a() -> str:
    return 34


def fn34_b(flag: bool) -> int:
    if flag:
        return "s34"
    return 2.5


def fn34_c() -> None:
    return 34


class Box34:
    def get(self) -> list[int]:
        return "items34"

    def size(self) -> dict[str, int]:
        return (1, 34)


def fn35_a() -> str:
    return 35


def fn35_b(flag: bool) -> int:
    if flag:
        return "s35"
    return 2.5


def fn35_c() -> None:
    return 35


class Box35:
    def get(self) -> list[int]:
        return "items35"

    def size(self) -> dict[str, int]:
        return (1, 35)


def fn36_a() -> str:
    return 36


def fn36_b(flag: bool) -> int:
    if flag:
        return "s36"
    return 2.5


def fn36_c() -> None:
    return 36


class Box36:
    def get(self) -> list[int]:
        return "items36"

    def size(self) -> dict[str, int]:
        return (1, 36)


def fn37_a() -> str:
    return 37


def fn37_b(flag: bool) -> int:
    if flag:
        return "s37"
    return 2.5


def fn37_c() -> None:
    return 37


class Box37:
    def get(self) -> list[int]:
        return "items37"

    def size(self) -> dict[str, int]:
        return (1, 37)


def fn38_a() -> str:
    return 38


def fn38_b(flag: bool) -> int:
    if flag:
        return "s38"
    return 2.5


def fn38_c() -> None:
    return 38


class Box38:
    def get(self) -> list[int]:
        return "items38"

    def size(self) -> dict[str, int]:
        return (1, 38)


def fn39_a() -> str:
    return 39


def fn39_b(flag: bool) -> int:
    if flag:
        return "s39"
    return 2.5


def fn39_c() -> None:
    return 39


class Box39:
    def get(self) -> list[int]:
        return "items39"

    def size(self) -> dict[str, int]:
        return (1, 39)


def fn40_a() -> str:
    return 40


def fn40_b(flag: bool) -> int:
    if flag:
        return "s40"
    return 2.5


def fn40_c() -> None:
    return 40


class Box40:
    def get(self) -> list[int]:
        return "items40"

    def size(self) -> dict[str, int]:
        return (1, 40)


def fn41_a() -> str:
    return 41


def fn41_b(flag: bool) -> int:
    if flag:
        return "s41"
    return 2.5


def fn41_c() -> None:
    return 41


class Box41:
    def get(self) -> list[int]:
        return "items41"

    def size(self) -> dict[str, int]:
        return (1, 41)


def fn42_a() -> str:
    return 42


def fn42_b(flag: bool) -> int:
    if flag:
        return "s42"
    return 2.5


def fn42_c() -> None:
    return 42


class Box42:
    def get(self) -> list[int]:
        return "items42"

    def size(self) -> dict[str, int]:
        return (1, 42)


def fn43_a() -> str:
    return 43


def fn43_b(flag: bool) -> int:
    if flag:
        return "s43"
    return 2.5


def fn43_c() -> None:
    return 43


class Box43:
    def get(self) -> list[int]:
        return "items43"

    def size(self) -> dict[str, int]:
        return (1, 43)


def fn44_a() -> str:
    return 44


def fn44_b(flag: bool) -> int:
    if flag:
        return "s44"
    return 2.5


def fn44_c() -> None:
    return 44


class Box44:
    def get(self) -> list[int]:
        return "items44"

    def size(self) -> dict[str, int]:
        return (1, 44)


def fn45_a() -> str:
    return 45


def fn45_b(flag: bool) -> int:
    if flag:
        return "s45"
    return 2.5


def fn45_c() -> None:
    return 45


class Box45:
    def get(self) -> list[int]:
        return "items45"

    def size(self) -> dict[str, int]:
        return (1, 45)


def fn46_a() -> str:
    return 46


def fn46_b(flag: bool) -> int:
    if flag:
        return "s46"
    return 2.5


def fn46_c() -> None:
    return 46


class Box46:
    def get(self) -> list[int]:
        return "items46"

    def size(self) -> dict[str, int]:
        return (1, 46)


def fn47_a() -> str:
    return 47


def fn47_b(flag: bool) -> int:
    if flag:
        return "s47"
    return 2.5


def fn47_c() -> None:
    return 47


class Box47:
    def get(self) -> list[int]:
        return "items47"

    def size(self) -> dict[str, int]:
        return (1, 47)


def fn48_a() -> str:
    return 48


def fn48_b(flag: bool) -> int:
    if flag:
        return "s48"
    return 2.5


def fn48_c() -> None:
    return 48


class Box48:
    def get(self) -> list[int]:
        return "items48"

    def size(self) -> dict[str, int]:
        return (1, 48)


def fn49_a() -> str:
    return 49


def fn49_b(flag: bool) -> int:
    if flag:
        return "s49"
    return 2.5


def fn49_c() -> None:
    return 49


class Box49:
    def get(self) -> list[int]:
        return "items49"

    def size(self) -> dict[str, int]:
        return (1, 49)


def fn50_a() -> str:
    return 50


def fn50_b(flag: bool) -> int:
    if flag:
        return "s50"
    return 2.5


def fn50_c() -> None:
    return 50


class Box50:
    def get(self) -> list[int]:
        return "items50"

    def size(self) -> dict[str, int]:
        return (1, 50)


def fn51_a() -> str:
    return 51


def fn51_b(flag: bool) -> int:
    if flag:
        return "s51"
    return 2.5


def fn51_c() -> None:
    return 51


class Box51:
    def get(self) -> list[int]:
        return "items51"

    def size(self) -> dict[str, int]:
        return (1, 51)


def fn52_a() -> str:
    return 52


def fn52_b(flag: bool) -> int:
    if flag:
        return "s52"
    return 2.5


def fn52_c() -> None:
    return 52


class Box52:
    def get(self) -> list[int]:
        return "items52"

    def size(self) -> dict[str, int]:
        return (1, 52)


def fn53_a() -> str:
    return 53


def fn53_b(flag: bool) -> int:
    if flag:
        return "s53"
    return 2.5


def fn53_c() -> None:
    return 53


class Box53:
    def get(self) -> list[int]:
        return "items53"

    def size(self) -> dict[str, int]:
        return (1, 53)


def fn54_a() -> str:
    return 54


def fn54_b(flag: bool) -> int:
    if flag:
        return "s54"
    return 2.5


def fn54_c() -> None:
    return 54


class Box54:
    def get(self) -> list[int]:
        return "items54"

    def size(self) -> dict[str, int]:
        return (1, 54)


def fn55_a() -> str:
    return 55


def fn55_b(flag: bool) -> int:
    if flag:
        return "s55"
    return 2.5


def fn55_c() -> None:
    return 55


class Box55:
    def get(self) -> list[int]:
        return "items55"

    def size(self) -> dict[str, int]:
        return (1, 55)


def fn56_a() -> str:
    return 56


def fn56_b(flag: bool) -> int:
    if flag:
        return "s56"
    return 2.5


def fn56_c() -> None:
    return 56


class Box56:
    def get(self) -> list[int]:
        return "items56"

    def size(self) -> dict[str, int]:
        return (1, 56)


def fn57_a() -> str:
    return 57


def fn57_b(flag: bool) -> int:
    if flag:
        return "s57"
    return 2.5


def fn57_c() -> None:
    return 57


class Box57:
    def get(self) -> list[int]:
        return "items57"

    def size(self) -> dict[str, int]:
        return (1, 57)


def fn58_a() -> str:
    return 58


def fn58_b(flag: bool) -> int:
    if flag:
        return "s58"
    return 2.5


def fn58_c() -> None:
    return 58


class Box58:
    def get(self) -> list[int]:
        return "items58"

    def size(self) -> dict[str, int]:
        return (1, 58)


def fn59_a() -> str:
    return 59


def fn59_b(flag: bool) -> int:
    if flag:
        return "s59"
    return 2.5


def fn59_c() -> None:
    return 59


class Box59:
    def get(self) -> list[int]:
        return "items59"

    def size(self) -> dict[str, int]:
        return (1, 59)


def fn60_a() -> str:
    return 60


def fn60_b(flag: bool) -> int:
    if flag:
        return "s60"
    return 2.5


def fn60_c() -> None:
    return 60


class Box60:
    def get(self) -> list[int]:
        return "items60"

    def size(self) -> dict[str, int]:
        return (1, 60)


def fn61_a() -> str:
    return 61


def fn61_b(flag: bool) -> int:
    if flag:
        return "s61"
    return 2.5


def fn61_c() -> None:
    return 61


class Box61:
    def get(self) -> list[int]:
        return "items61"

    def size(self) -> dict[str, int]:
        return (1, 61)


def fn62_a() -> str:
    return 62


def fn62_b(flag: bool) -> int:
    if flag:
        return "s62"
    return 2.5


def fn62_c() -> None:
    return 62


class Box62:
    def get(self) -> list[int]:
        return "items62"

    def size(self) -> dict[str, int]:
        return (1, 62)


def fn63_a() -> str:
    return 63


def fn63_b(flag: bool) -> int:
    if flag:
        return "s63"
    return 2.5


def fn63_c() -> None:
    return 63


class Box63:
    def get(self) -> list[int]:
        return "items63"

    def size(self) -> dict[str, int]:
        return (1, 63)


def fn64_a() -> str:
    return 64


def fn64_b(flag: bool) -> int:
    if flag:
        return "s64"
    return 2.5


def fn64_c() -> None:
    return 64


class Box64:
    def get(self) -> list[int]:
        return "items64"

    def size(self) -> dict[str, int]:
        return (1, 64)


def fn65_a() -> str:
    return 65


def fn65_b(flag: bool) -> int:
    if flag:
        return "s65"
    return 2.5


def fn65_c() -> None:
    return 65


class Box65:
    def get(self) -> list[int]:
        return "items65"

    def size(self) -> dict[str, int]:
        return (1, 65)


def fn66_a() -> str:
    return 66


def fn66_b(flag: bool) -> int:
    if flag:
        return "s66"
    return 2.5


def fn66_c() -> None:
    return 66


class Box66:
    def get(self) -> list[int]:
        return "items66"

    def size(self) -> dict[str, int]:
        return (1, 66)


def fn67_a() -> str:
    return 67


def fn67_b(flag: bool) -> int:
    if flag:
        return "s67"
    return 2.5


def fn67_c() -> None:
    return 67


class Box67:
    def get(self) -> list[int]:
        return "items67"

    def size(self) -> dict[str, int]:
        return (1, 67)


def fn68_a() -> str:
    return 68


def fn68_b(flag: bool) -> int:
    if flag:
        return "s68"
    return 2.5


def fn68_c() -> None:
    return 68


class Box68:
    def get(self) -> list[int]:
        return "items68"

    def size(self) -> dict[str, int]:
        return (1, 68)


def fn69_a() -> str:
    return 69


def fn69_b(flag: bool) -> int:
    if flag:
        return "s69"
    return 2.5


def fn69_c() -> None:
    return 69


class Box69:
    def get(self) -> list[int]:
        return "items69"

    def size(self) -> dict[str, int]:
        return (1, 69)


def fn70_a() -> str:
    return 70


def fn70_b(flag: bool) -> int:
    if flag:
        return "s70"
    return 2.5


def fn70_c() -> None:
    return 70


class Box70:
    def get(self) -> list[int]:
        return "items70"

    def size(self) -> dict[str, int]:
        return (1, 70)


def fn71_a() -> str:
    return 71


def fn71_b(flag: bool) -> int:
    if flag:
        return "s71"
    return 2.5


def fn71_c() -> None:
    return 71


class Box71:
    def get(self) -> list[int]:
        return "items71"

    def size(self) -> dict[str, int]:
        return (1, 71)


def fn72_a() -> str:
    return 72


def fn72_b(flag: bool) -> int:
    if flag:
        return "s72"
    return 2.5


def fn72_c() -> None:
    return 72


class Box72:
    def get(self) -> list[int]:
        return "items72"

    def size(self) -> dict[str, int]:
        return (1, 72)


def fn73_a() -> str:
    return 73


def fn73_b(flag: bool) -> int:
    if flag:
        return "s73"
    return 2.5


def fn73_c() -> None:
    return 73


class Box73:
    def get(self) -> list[int]:
        return "items73"

    def size(self) -> dict[str, int]:
        return (1, 73)


def fn74_a() -> str:
    return 74


def fn74_b(flag: bool) -> int:
    if flag:
        return "s74"
    return 2.5


def fn74_c() -> None:
    return 74


class Box74:
    def get(self) -> list[int]:
        return "items74"

    def size(self) -> dict[str, int]:
        return (1, 74)


def fn75_a() -> str:
    return 75


def fn75_b(flag: bool) -> int:
    if flag:
        return "s75"
    return 2.5


def fn75_c() -> None:
    return 75


class Box75:
    def get(self) -> list[int]:
        return "items75"

    def size(self) -> dict[str, int]:
        return (1, 75)


def fn76_a() -> str:
    return 76


def fn76_b(flag: bool) -> int:
    if flag:
        return "s76"
    return 2.5


def fn76_c() -> None:
    return 76


class Box76:
    def get(self) -> list[int]:
        return "items76"

    def size(self) -> dict[str, int]:
        return (1, 76)


def fn77_a() -> str:
    return 77


def fn77_b(flag: bool) -> int:
    if flag:
        return "s77"
    return 2.5


def fn77_c() -> None:
    return 77


class Box77:
    def get(self) -> list[int]:
        return "items77"

    def size(self) -> dict[str, int]:
        return (1, 77)


def fn78_a() -> str:
    return 78


def fn78_b(flag: bool) -> int:
    if flag:
        return "s78"
    return 2.5


def fn78_c() -> None:
    return 78


class Box78:
    def get(self) -> list[int]:
        return "items78"

    def size(self) -> dict[str, int]:
        return (1, 78)


def fn79_a() -> str:
    return 79


def fn79_b(flag: bool) -> int:
    if flag:
        return "s79"
    return 2.5


def fn79_c() -> None:
    return 79


class Box79:
    def get(self) -> list[int]:
        return "items79"

    def size(self) -> dict[str, int]:
        return (1, 79)


def fn80_a() -> str:
    return 80


def fn80_b(flag: bool) -> int:
    if flag:
        return "s80"
    return 2.5


def fn80_c() -> None:
    return 80


class Box80:
    def get(self) -> list[int]:
        return "items80"

    def size(self) -> dict[str, int]:
        return (1, 80)


def fn81_a() -> str:
    return 81


def fn81_b(flag: bool) -> int:
    if flag:
        return "s81"
    return 2.5


def fn81_c() -> None:
    return 81


class Box81:
    def get(self) -> list[int]:
        return "items81"

    def size(self) -> dict[str, int]:
        return (1, 81)


def fn82_a() -> str:
    return 82


def fn82_b(flag: bool) -> int:
    if flag:
        return "s82"
    return 2.5


def fn82_c() -> None:
    return 82


class Box82:
    def get(self) -> list[int]:
        return "items82"

    def size(self) -> dict[str, int]:
        return (1, 82)


def fn83_a() -> str:
    return 83


def fn83_b(flag: bool) -> int:
    if flag:
        return "s83"
    return 2.5


def fn83_c() -> None:
    return 83


class Box83:
    def get(self) -> list[int]:
        return "items83"

    def size(self) -> dict[str, int]:
        return (1, 83)


def fn84_a() -> str:
    return 84


def fn84_b(flag: bool) -> int:
    if flag:
        return "s84"
    return 2.5


def fn84_c() -> None:
    return 84


class Box84:
    def get(self) -> list[int]:
        return "items84"

    def size(self) -> dict[str, int]:
        return (1, 84)


def fn85_a() -> str:
    return 85


def fn85_b(flag: bool) -> int:
    if flag:
        return "s85"
    return 2.5


def fn85_c() -> None:
    return 85


class Box85:
    def get(self) -> list[int]:
        return "items85"

    def size(self) -> dict[str, int]:
        return (1, 85)


def fn86_a() -> str:
    return 86


def fn86_b(flag: bool) -> int:
    if flag:
        return "s86"
    return 2.5


def fn86_c() -> None:
    return 86


class Box86:
    def get(self) -> list[int]:
        return "items86"

    def size(self) -> dict[str, int]:
        return (1, 86)


def fn87_a() -> str:
    return 87


def fn87_b(flag: bool) -> int:
    if flag:
        return "s87"
    return 2.5


def fn87_c() -> None:
    return 87


class Box87:
    def get(self) -> list[int]:
        return "items87"

    def size(self) -> dict[str, int]:
        return (1, 87)


def fn88_a() -> str:
    return 88


def fn88_b(flag: bool) -> int:
    if flag:
        return "s88"
    return 2.5


def fn88_c() -> None:
    return 88


class Box88:
    def get(self) -> list[int]:
        return "items88"

    def size(self) -> dict[str, int]:
        return (1, 88)


def fn89_a() -> str:
    return 89


def fn89_b(flag: bool) -> int:
    if flag:
        return "s89"
    return 2.5


def fn89_c() -> None:
    return 89


class Box89:
    def get(self) -> list[int]:
        return "items89"

    def size(self) -> dict[str, int]:
        return (1, 89)
