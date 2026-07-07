from typing import Callable

var0 = 1
type GoodA0 = int | None
type GoodB0 = dict[str, list[int]]
type GoodC0 = Callable[[int], str]
type BadA0 = [int, str]
type BadB0 = True
type BadC0 = 42
def use0(x: GoodA0, y: GoodB0) -> GoodC0 | None:
    return None

var1 = 1
type GoodA1 = int | None
type GoodB1 = dict[str, list[int]]
type GoodC1 = Callable[[int], str]
type BadA1 = {"a": "b"}
type BadB1 = var1
type BadC1 = int if 1 < 3 else str
def use1(x: GoodA1, y: GoodB1) -> GoodC1 | None:
    return None

var2 = 1
type GoodA2 = int | None
type GoodB2 = dict[str, list[int]]
type GoodC2 = Callable[[int], str]
type BadA2 = f"{'int'}"
type BadB2 = (int, str)
type BadC2 = list or set
def use2(x: GoodA2, y: GoodB2) -> GoodC2 | None:
    return None

var3 = 1
type GoodA3 = int | None
type GoodB3 = dict[str, list[int]]
type GoodC3 = Callable[[int], str]
type BadA3 = (lambda: int)()
type BadB3 = -1
type BadC3 = [int][0]
def use3(x: GoodA3, y: GoodB3) -> GoodC3 | None:
    return None

var4 = 1
type GoodA4 = int | None
type GoodB4 = dict[str, list[int]]
type GoodC4 = Callable[[int], str]
type BadA4 = [int, str]
type BadB4 = True
type BadC4 = 42
def use4(x: GoodA4, y: GoodB4) -> GoodC4 | None:
    return None

var5 = 1
type GoodA5 = int | None
type GoodB5 = dict[str, list[int]]
type GoodC5 = Callable[[int], str]
type BadA5 = {"a": "b"}
type BadB5 = var5
type BadC5 = int if 1 < 3 else str
def use5(x: GoodA5, y: GoodB5) -> GoodC5 | None:
    return None

var6 = 1
type GoodA6 = int | None
type GoodB6 = dict[str, list[int]]
type GoodC6 = Callable[[int], str]
type BadA6 = f"{'int'}"
type BadB6 = (int, str)
type BadC6 = list or set
def use6(x: GoodA6, y: GoodB6) -> GoodC6 | None:
    return None

var7 = 1
type GoodA7 = int | None
type GoodB7 = dict[str, list[int]]
type GoodC7 = Callable[[int], str]
type BadA7 = (lambda: int)()
type BadB7 = -1
type BadC7 = [int][0]
def use7(x: GoodA7, y: GoodB7) -> GoodC7 | None:
    return None

var8 = 1
type GoodA8 = int | None
type GoodB8 = dict[str, list[int]]
type GoodC8 = Callable[[int], str]
type BadA8 = [int, str]
type BadB8 = True
type BadC8 = 42
def use8(x: GoodA8, y: GoodB8) -> GoodC8 | None:
    return None

var9 = 1
type GoodA9 = int | None
type GoodB9 = dict[str, list[int]]
type GoodC9 = Callable[[int], str]
type BadA9 = {"a": "b"}
type BadB9 = var9
type BadC9 = int if 1 < 3 else str
def use9(x: GoodA9, y: GoodB9) -> GoodC9 | None:
    return None

var10 = 1
type GoodA10 = int | None
type GoodB10 = dict[str, list[int]]
type GoodC10 = Callable[[int], str]
type BadA10 = f"{'int'}"
type BadB10 = (int, str)
type BadC10 = list or set
def use10(x: GoodA10, y: GoodB10) -> GoodC10 | None:
    return None

var11 = 1
type GoodA11 = int | None
type GoodB11 = dict[str, list[int]]
type GoodC11 = Callable[[int], str]
type BadA11 = (lambda: int)()
type BadB11 = -1
type BadC11 = [int][0]
def use11(x: GoodA11, y: GoodB11) -> GoodC11 | None:
    return None

var12 = 1
type GoodA12 = int | None
type GoodB12 = dict[str, list[int]]
type GoodC12 = Callable[[int], str]
type BadA12 = [int, str]
type BadB12 = True
type BadC12 = 42
def use12(x: GoodA12, y: GoodB12) -> GoodC12 | None:
    return None

var13 = 1
type GoodA13 = int | None
type GoodB13 = dict[str, list[int]]
type GoodC13 = Callable[[int], str]
type BadA13 = {"a": "b"}
type BadB13 = var13
type BadC13 = int if 1 < 3 else str
def use13(x: GoodA13, y: GoodB13) -> GoodC13 | None:
    return None

var14 = 1
type GoodA14 = int | None
type GoodB14 = dict[str, list[int]]
type GoodC14 = Callable[[int], str]
type BadA14 = f"{'int'}"
type BadB14 = (int, str)
type BadC14 = list or set
def use14(x: GoodA14, y: GoodB14) -> GoodC14 | None:
    return None

var15 = 1
type GoodA15 = int | None
type GoodB15 = dict[str, list[int]]
type GoodC15 = Callable[[int], str]
type BadA15 = (lambda: int)()
type BadB15 = -1
type BadC15 = [int][0]
def use15(x: GoodA15, y: GoodB15) -> GoodC15 | None:
    return None

var16 = 1
type GoodA16 = int | None
type GoodB16 = dict[str, list[int]]
type GoodC16 = Callable[[int], str]
type BadA16 = [int, str]
type BadB16 = True
type BadC16 = 42
def use16(x: GoodA16, y: GoodB16) -> GoodC16 | None:
    return None

var17 = 1
type GoodA17 = int | None
type GoodB17 = dict[str, list[int]]
type GoodC17 = Callable[[int], str]
type BadA17 = {"a": "b"}
type BadB17 = var17
type BadC17 = int if 1 < 3 else str
def use17(x: GoodA17, y: GoodB17) -> GoodC17 | None:
    return None

var18 = 1
type GoodA18 = int | None
type GoodB18 = dict[str, list[int]]
type GoodC18 = Callable[[int], str]
type BadA18 = f"{'int'}"
type BadB18 = (int, str)
type BadC18 = list or set
def use18(x: GoodA18, y: GoodB18) -> GoodC18 | None:
    return None

var19 = 1
type GoodA19 = int | None
type GoodB19 = dict[str, list[int]]
type GoodC19 = Callable[[int], str]
type BadA19 = (lambda: int)()
type BadB19 = -1
type BadC19 = [int][0]
def use19(x: GoodA19, y: GoodB19) -> GoodC19 | None:
    return None

var20 = 1
type GoodA20 = int | None
type GoodB20 = dict[str, list[int]]
type GoodC20 = Callable[[int], str]
type BadA20 = [int, str]
type BadB20 = True
type BadC20 = 42
def use20(x: GoodA20, y: GoodB20) -> GoodC20 | None:
    return None

var21 = 1
type GoodA21 = int | None
type GoodB21 = dict[str, list[int]]
type GoodC21 = Callable[[int], str]
type BadA21 = {"a": "b"}
type BadB21 = var21
type BadC21 = int if 1 < 3 else str
def use21(x: GoodA21, y: GoodB21) -> GoodC21 | None:
    return None

var22 = 1
type GoodA22 = int | None
type GoodB22 = dict[str, list[int]]
type GoodC22 = Callable[[int], str]
type BadA22 = f"{'int'}"
type BadB22 = (int, str)
type BadC22 = list or set
def use22(x: GoodA22, y: GoodB22) -> GoodC22 | None:
    return None

var23 = 1
type GoodA23 = int | None
type GoodB23 = dict[str, list[int]]
type GoodC23 = Callable[[int], str]
type BadA23 = (lambda: int)()
type BadB23 = -1
type BadC23 = [int][0]
def use23(x: GoodA23, y: GoodB23) -> GoodC23 | None:
    return None

var24 = 1
type GoodA24 = int | None
type GoodB24 = dict[str, list[int]]
type GoodC24 = Callable[[int], str]
type BadA24 = [int, str]
type BadB24 = True
type BadC24 = 42
def use24(x: GoodA24, y: GoodB24) -> GoodC24 | None:
    return None

var25 = 1
type GoodA25 = int | None
type GoodB25 = dict[str, list[int]]
type GoodC25 = Callable[[int], str]
type BadA25 = {"a": "b"}
type BadB25 = var25
type BadC25 = int if 1 < 3 else str
def use25(x: GoodA25, y: GoodB25) -> GoodC25 | None:
    return None

var26 = 1
type GoodA26 = int | None
type GoodB26 = dict[str, list[int]]
type GoodC26 = Callable[[int], str]
type BadA26 = f"{'int'}"
type BadB26 = (int, str)
type BadC26 = list or set
def use26(x: GoodA26, y: GoodB26) -> GoodC26 | None:
    return None

var27 = 1
type GoodA27 = int | None
type GoodB27 = dict[str, list[int]]
type GoodC27 = Callable[[int], str]
type BadA27 = (lambda: int)()
type BadB27 = -1
type BadC27 = [int][0]
def use27(x: GoodA27, y: GoodB27) -> GoodC27 | None:
    return None

var28 = 1
type GoodA28 = int | None
type GoodB28 = dict[str, list[int]]
type GoodC28 = Callable[[int], str]
type BadA28 = [int, str]
type BadB28 = True
type BadC28 = 42
def use28(x: GoodA28, y: GoodB28) -> GoodC28 | None:
    return None

var29 = 1
type GoodA29 = int | None
type GoodB29 = dict[str, list[int]]
type GoodC29 = Callable[[int], str]
type BadA29 = {"a": "b"}
type BadB29 = var29
type BadC29 = int if 1 < 3 else str
def use29(x: GoodA29, y: GoodB29) -> GoodC29 | None:
    return None

var30 = 1
type GoodA30 = int | None
type GoodB30 = dict[str, list[int]]
type GoodC30 = Callable[[int], str]
type BadA30 = f"{'int'}"
type BadB30 = (int, str)
type BadC30 = list or set
def use30(x: GoodA30, y: GoodB30) -> GoodC30 | None:
    return None

var31 = 1
type GoodA31 = int | None
type GoodB31 = dict[str, list[int]]
type GoodC31 = Callable[[int], str]
type BadA31 = (lambda: int)()
type BadB31 = -1
type BadC31 = [int][0]
def use31(x: GoodA31, y: GoodB31) -> GoodC31 | None:
    return None

var32 = 1
type GoodA32 = int | None
type GoodB32 = dict[str, list[int]]
type GoodC32 = Callable[[int], str]
type BadA32 = [int, str]
type BadB32 = True
type BadC32 = 42
def use32(x: GoodA32, y: GoodB32) -> GoodC32 | None:
    return None

var33 = 1
type GoodA33 = int | None
type GoodB33 = dict[str, list[int]]
type GoodC33 = Callable[[int], str]
type BadA33 = {"a": "b"}
type BadB33 = var33
type BadC33 = int if 1 < 3 else str
def use33(x: GoodA33, y: GoodB33) -> GoodC33 | None:
    return None

var34 = 1
type GoodA34 = int | None
type GoodB34 = dict[str, list[int]]
type GoodC34 = Callable[[int], str]
type BadA34 = f"{'int'}"
type BadB34 = (int, str)
type BadC34 = list or set
def use34(x: GoodA34, y: GoodB34) -> GoodC34 | None:
    return None

var35 = 1
type GoodA35 = int | None
type GoodB35 = dict[str, list[int]]
type GoodC35 = Callable[[int], str]
type BadA35 = (lambda: int)()
type BadB35 = -1
type BadC35 = [int][0]
def use35(x: GoodA35, y: GoodB35) -> GoodC35 | None:
    return None

var36 = 1
type GoodA36 = int | None
type GoodB36 = dict[str, list[int]]
type GoodC36 = Callable[[int], str]
type BadA36 = [int, str]
type BadB36 = True
type BadC36 = 42
def use36(x: GoodA36, y: GoodB36) -> GoodC36 | None:
    return None

var37 = 1
type GoodA37 = int | None
type GoodB37 = dict[str, list[int]]
type GoodC37 = Callable[[int], str]
type BadA37 = {"a": "b"}
type BadB37 = var37
type BadC37 = int if 1 < 3 else str
def use37(x: GoodA37, y: GoodB37) -> GoodC37 | None:
    return None

var38 = 1
type GoodA38 = int | None
type GoodB38 = dict[str, list[int]]
type GoodC38 = Callable[[int], str]
type BadA38 = f"{'int'}"
type BadB38 = (int, str)
type BadC38 = list or set
def use38(x: GoodA38, y: GoodB38) -> GoodC38 | None:
    return None

var39 = 1
type GoodA39 = int | None
type GoodB39 = dict[str, list[int]]
type GoodC39 = Callable[[int], str]
type BadA39 = (lambda: int)()
type BadB39 = -1
type BadC39 = [int][0]
def use39(x: GoodA39, y: GoodB39) -> GoodC39 | None:
    return None

var40 = 1
type GoodA40 = int | None
type GoodB40 = dict[str, list[int]]
type GoodC40 = Callable[[int], str]
type BadA40 = [int, str]
type BadB40 = True
type BadC40 = 42
def use40(x: GoodA40, y: GoodB40) -> GoodC40 | None:
    return None

var41 = 1
type GoodA41 = int | None
type GoodB41 = dict[str, list[int]]
type GoodC41 = Callable[[int], str]
type BadA41 = {"a": "b"}
type BadB41 = var41
type BadC41 = int if 1 < 3 else str
def use41(x: GoodA41, y: GoodB41) -> GoodC41 | None:
    return None

var42 = 1
type GoodA42 = int | None
type GoodB42 = dict[str, list[int]]
type GoodC42 = Callable[[int], str]
type BadA42 = f"{'int'}"
type BadB42 = (int, str)
type BadC42 = list or set
def use42(x: GoodA42, y: GoodB42) -> GoodC42 | None:
    return None

var43 = 1
type GoodA43 = int | None
type GoodB43 = dict[str, list[int]]
type GoodC43 = Callable[[int], str]
type BadA43 = (lambda: int)()
type BadB43 = -1
type BadC43 = [int][0]
def use43(x: GoodA43, y: GoodB43) -> GoodC43 | None:
    return None

var44 = 1
type GoodA44 = int | None
type GoodB44 = dict[str, list[int]]
type GoodC44 = Callable[[int], str]
type BadA44 = [int, str]
type BadB44 = True
type BadC44 = 42
def use44(x: GoodA44, y: GoodB44) -> GoodC44 | None:
    return None

var45 = 1
type GoodA45 = int | None
type GoodB45 = dict[str, list[int]]
type GoodC45 = Callable[[int], str]
type BadA45 = {"a": "b"}
type BadB45 = var45
type BadC45 = int if 1 < 3 else str
def use45(x: GoodA45, y: GoodB45) -> GoodC45 | None:
    return None

var46 = 1
type GoodA46 = int | None
type GoodB46 = dict[str, list[int]]
type GoodC46 = Callable[[int], str]
type BadA46 = f"{'int'}"
type BadB46 = (int, str)
type BadC46 = list or set
def use46(x: GoodA46, y: GoodB46) -> GoodC46 | None:
    return None

var47 = 1
type GoodA47 = int | None
type GoodB47 = dict[str, list[int]]
type GoodC47 = Callable[[int], str]
type BadA47 = (lambda: int)()
type BadB47 = -1
type BadC47 = [int][0]
def use47(x: GoodA47, y: GoodB47) -> GoodC47 | None:
    return None

var48 = 1
type GoodA48 = int | None
type GoodB48 = dict[str, list[int]]
type GoodC48 = Callable[[int], str]
type BadA48 = [int, str]
type BadB48 = True
type BadC48 = 42
def use48(x: GoodA48, y: GoodB48) -> GoodC48 | None:
    return None

var49 = 1
type GoodA49 = int | None
type GoodB49 = dict[str, list[int]]
type GoodC49 = Callable[[int], str]
type BadA49 = {"a": "b"}
type BadB49 = var49
type BadC49 = int if 1 < 3 else str
def use49(x: GoodA49, y: GoodB49) -> GoodC49 | None:
    return None

var50 = 1
type GoodA50 = int | None
type GoodB50 = dict[str, list[int]]
type GoodC50 = Callable[[int], str]
type BadA50 = f"{'int'}"
type BadB50 = (int, str)
type BadC50 = list or set
def use50(x: GoodA50, y: GoodB50) -> GoodC50 | None:
    return None

var51 = 1
type GoodA51 = int | None
type GoodB51 = dict[str, list[int]]
type GoodC51 = Callable[[int], str]
type BadA51 = (lambda: int)()
type BadB51 = -1
type BadC51 = [int][0]
def use51(x: GoodA51, y: GoodB51) -> GoodC51 | None:
    return None

var52 = 1
type GoodA52 = int | None
type GoodB52 = dict[str, list[int]]
type GoodC52 = Callable[[int], str]
type BadA52 = [int, str]
type BadB52 = True
type BadC52 = 42
def use52(x: GoodA52, y: GoodB52) -> GoodC52 | None:
    return None

var53 = 1
type GoodA53 = int | None
type GoodB53 = dict[str, list[int]]
type GoodC53 = Callable[[int], str]
type BadA53 = {"a": "b"}
type BadB53 = var53
type BadC53 = int if 1 < 3 else str
def use53(x: GoodA53, y: GoodB53) -> GoodC53 | None:
    return None

var54 = 1
type GoodA54 = int | None
type GoodB54 = dict[str, list[int]]
type GoodC54 = Callable[[int], str]
type BadA54 = f"{'int'}"
type BadB54 = (int, str)
type BadC54 = list or set
def use54(x: GoodA54, y: GoodB54) -> GoodC54 | None:
    return None

var55 = 1
type GoodA55 = int | None
type GoodB55 = dict[str, list[int]]
type GoodC55 = Callable[[int], str]
type BadA55 = (lambda: int)()
type BadB55 = -1
type BadC55 = [int][0]
def use55(x: GoodA55, y: GoodB55) -> GoodC55 | None:
    return None

var56 = 1
type GoodA56 = int | None
type GoodB56 = dict[str, list[int]]
type GoodC56 = Callable[[int], str]
type BadA56 = [int, str]
type BadB56 = True
type BadC56 = 42
def use56(x: GoodA56, y: GoodB56) -> GoodC56 | None:
    return None

var57 = 1
type GoodA57 = int | None
type GoodB57 = dict[str, list[int]]
type GoodC57 = Callable[[int], str]
type BadA57 = {"a": "b"}
type BadB57 = var57
type BadC57 = int if 1 < 3 else str
def use57(x: GoodA57, y: GoodB57) -> GoodC57 | None:
    return None

var58 = 1
type GoodA58 = int | None
type GoodB58 = dict[str, list[int]]
type GoodC58 = Callable[[int], str]
type BadA58 = f"{'int'}"
type BadB58 = (int, str)
type BadC58 = list or set
def use58(x: GoodA58, y: GoodB58) -> GoodC58 | None:
    return None

var59 = 1
type GoodA59 = int | None
type GoodB59 = dict[str, list[int]]
type GoodC59 = Callable[[int], str]
type BadA59 = (lambda: int)()
type BadB59 = -1
type BadC59 = [int][0]
def use59(x: GoodA59, y: GoodB59) -> GoodC59 | None:
    return None

var60 = 1
type GoodA60 = int | None
type GoodB60 = dict[str, list[int]]
type GoodC60 = Callable[[int], str]
type BadA60 = [int, str]
type BadB60 = True
type BadC60 = 42
def use60(x: GoodA60, y: GoodB60) -> GoodC60 | None:
    return None

var61 = 1
type GoodA61 = int | None
type GoodB61 = dict[str, list[int]]
type GoodC61 = Callable[[int], str]
type BadA61 = {"a": "b"}
type BadB61 = var61
type BadC61 = int if 1 < 3 else str
def use61(x: GoodA61, y: GoodB61) -> GoodC61 | None:
    return None

var62 = 1
type GoodA62 = int | None
type GoodB62 = dict[str, list[int]]
type GoodC62 = Callable[[int], str]
type BadA62 = f"{'int'}"
type BadB62 = (int, str)
type BadC62 = list or set
def use62(x: GoodA62, y: GoodB62) -> GoodC62 | None:
    return None

var63 = 1
type GoodA63 = int | None
type GoodB63 = dict[str, list[int]]
type GoodC63 = Callable[[int], str]
type BadA63 = (lambda: int)()
type BadB63 = -1
type BadC63 = [int][0]
def use63(x: GoodA63, y: GoodB63) -> GoodC63 | None:
    return None

var64 = 1
type GoodA64 = int | None
type GoodB64 = dict[str, list[int]]
type GoodC64 = Callable[[int], str]
type BadA64 = [int, str]
type BadB64 = True
type BadC64 = 42
def use64(x: GoodA64, y: GoodB64) -> GoodC64 | None:
    return None

var65 = 1
type GoodA65 = int | None
type GoodB65 = dict[str, list[int]]
type GoodC65 = Callable[[int], str]
type BadA65 = {"a": "b"}
type BadB65 = var65
type BadC65 = int if 1 < 3 else str
def use65(x: GoodA65, y: GoodB65) -> GoodC65 | None:
    return None

var66 = 1
type GoodA66 = int | None
type GoodB66 = dict[str, list[int]]
type GoodC66 = Callable[[int], str]
type BadA66 = f"{'int'}"
type BadB66 = (int, str)
type BadC66 = list or set
def use66(x: GoodA66, y: GoodB66) -> GoodC66 | None:
    return None

var67 = 1
type GoodA67 = int | None
type GoodB67 = dict[str, list[int]]
type GoodC67 = Callable[[int], str]
type BadA67 = (lambda: int)()
type BadB67 = -1
type BadC67 = [int][0]
def use67(x: GoodA67, y: GoodB67) -> GoodC67 | None:
    return None

var68 = 1
type GoodA68 = int | None
type GoodB68 = dict[str, list[int]]
type GoodC68 = Callable[[int], str]
type BadA68 = [int, str]
type BadB68 = True
type BadC68 = 42
def use68(x: GoodA68, y: GoodB68) -> GoodC68 | None:
    return None

var69 = 1
type GoodA69 = int | None
type GoodB69 = dict[str, list[int]]
type GoodC69 = Callable[[int], str]
type BadA69 = {"a": "b"}
type BadB69 = var69
type BadC69 = int if 1 < 3 else str
def use69(x: GoodA69, y: GoodB69) -> GoodC69 | None:
    return None

var70 = 1
type GoodA70 = int | None
type GoodB70 = dict[str, list[int]]
type GoodC70 = Callable[[int], str]
type BadA70 = f"{'int'}"
type BadB70 = (int, str)
type BadC70 = list or set
def use70(x: GoodA70, y: GoodB70) -> GoodC70 | None:
    return None

var71 = 1
type GoodA71 = int | None
type GoodB71 = dict[str, list[int]]
type GoodC71 = Callable[[int], str]
type BadA71 = (lambda: int)()
type BadB71 = -1
type BadC71 = [int][0]
def use71(x: GoodA71, y: GoodB71) -> GoodC71 | None:
    return None

var72 = 1
type GoodA72 = int | None
type GoodB72 = dict[str, list[int]]
type GoodC72 = Callable[[int], str]
type BadA72 = [int, str]
type BadB72 = True
type BadC72 = 42
def use72(x: GoodA72, y: GoodB72) -> GoodC72 | None:
    return None

var73 = 1
type GoodA73 = int | None
type GoodB73 = dict[str, list[int]]
type GoodC73 = Callable[[int], str]
type BadA73 = {"a": "b"}
type BadB73 = var73
type BadC73 = int if 1 < 3 else str
def use73(x: GoodA73, y: GoodB73) -> GoodC73 | None:
    return None

var74 = 1
type GoodA74 = int | None
type GoodB74 = dict[str, list[int]]
type GoodC74 = Callable[[int], str]
type BadA74 = f"{'int'}"
type BadB74 = (int, str)
type BadC74 = list or set
def use74(x: GoodA74, y: GoodB74) -> GoodC74 | None:
    return None

var75 = 1
type GoodA75 = int | None
type GoodB75 = dict[str, list[int]]
type GoodC75 = Callable[[int], str]
type BadA75 = (lambda: int)()
type BadB75 = -1
type BadC75 = [int][0]
def use75(x: GoodA75, y: GoodB75) -> GoodC75 | None:
    return None

var76 = 1
type GoodA76 = int | None
type GoodB76 = dict[str, list[int]]
type GoodC76 = Callable[[int], str]
type BadA76 = [int, str]
type BadB76 = True
type BadC76 = 42
def use76(x: GoodA76, y: GoodB76) -> GoodC76 | None:
    return None

var77 = 1
type GoodA77 = int | None
type GoodB77 = dict[str, list[int]]
type GoodC77 = Callable[[int], str]
type BadA77 = {"a": "b"}
type BadB77 = var77
type BadC77 = int if 1 < 3 else str
def use77(x: GoodA77, y: GoodB77) -> GoodC77 | None:
    return None

var78 = 1
type GoodA78 = int | None
type GoodB78 = dict[str, list[int]]
type GoodC78 = Callable[[int], str]
type BadA78 = f"{'int'}"
type BadB78 = (int, str)
type BadC78 = list or set
def use78(x: GoodA78, y: GoodB78) -> GoodC78 | None:
    return None

var79 = 1
type GoodA79 = int | None
type GoodB79 = dict[str, list[int]]
type GoodC79 = Callable[[int], str]
type BadA79 = (lambda: int)()
type BadB79 = -1
type BadC79 = [int][0]
def use79(x: GoodA79, y: GoodB79) -> GoodC79 | None:
    return None

var80 = 1
type GoodA80 = int | None
type GoodB80 = dict[str, list[int]]
type GoodC80 = Callable[[int], str]
type BadA80 = [int, str]
type BadB80 = True
type BadC80 = 42
def use80(x: GoodA80, y: GoodB80) -> GoodC80 | None:
    return None

var81 = 1
type GoodA81 = int | None
type GoodB81 = dict[str, list[int]]
type GoodC81 = Callable[[int], str]
type BadA81 = {"a": "b"}
type BadB81 = var81
type BadC81 = int if 1 < 3 else str
def use81(x: GoodA81, y: GoodB81) -> GoodC81 | None:
    return None

var82 = 1
type GoodA82 = int | None
type GoodB82 = dict[str, list[int]]
type GoodC82 = Callable[[int], str]
type BadA82 = f"{'int'}"
type BadB82 = (int, str)
type BadC82 = list or set
def use82(x: GoodA82, y: GoodB82) -> GoodC82 | None:
    return None

var83 = 1
type GoodA83 = int | None
type GoodB83 = dict[str, list[int]]
type GoodC83 = Callable[[int], str]
type BadA83 = (lambda: int)()
type BadB83 = -1
type BadC83 = [int][0]
def use83(x: GoodA83, y: GoodB83) -> GoodC83 | None:
    return None

var84 = 1
type GoodA84 = int | None
type GoodB84 = dict[str, list[int]]
type GoodC84 = Callable[[int], str]
type BadA84 = [int, str]
type BadB84 = True
type BadC84 = 42
def use84(x: GoodA84, y: GoodB84) -> GoodC84 | None:
    return None

var85 = 1
type GoodA85 = int | None
type GoodB85 = dict[str, list[int]]
type GoodC85 = Callable[[int], str]
type BadA85 = {"a": "b"}
type BadB85 = var85
type BadC85 = int if 1 < 3 else str
def use85(x: GoodA85, y: GoodB85) -> GoodC85 | None:
    return None

var86 = 1
type GoodA86 = int | None
type GoodB86 = dict[str, list[int]]
type GoodC86 = Callable[[int], str]
type BadA86 = f"{'int'}"
type BadB86 = (int, str)
type BadC86 = list or set
def use86(x: GoodA86, y: GoodB86) -> GoodC86 | None:
    return None

var87 = 1
type GoodA87 = int | None
type GoodB87 = dict[str, list[int]]
type GoodC87 = Callable[[int], str]
type BadA87 = (lambda: int)()
type BadB87 = -1
type BadC87 = [int][0]
def use87(x: GoodA87, y: GoodB87) -> GoodC87 | None:
    return None

var88 = 1
type GoodA88 = int | None
type GoodB88 = dict[str, list[int]]
type GoodC88 = Callable[[int], str]
type BadA88 = [int, str]
type BadB88 = True
type BadC88 = 42
def use88(x: GoodA88, y: GoodB88) -> GoodC88 | None:
    return None

var89 = 1
type GoodA89 = int | None
type GoodB89 = dict[str, list[int]]
type GoodC89 = Callable[[int], str]
type BadA89 = {"a": "b"}
type BadB89 = var89
type BadC89 = int if 1 < 3 else str
def use89(x: GoodA89, y: GoodB89) -> GoodC89 | None:
    return None

var90 = 1
type GoodA90 = int | None
type GoodB90 = dict[str, list[int]]
type GoodC90 = Callable[[int], str]
type BadA90 = f"{'int'}"
type BadB90 = (int, str)
type BadC90 = list or set
def use90(x: GoodA90, y: GoodB90) -> GoodC90 | None:
    return None

var91 = 1
type GoodA91 = int | None
type GoodB91 = dict[str, list[int]]
type GoodC91 = Callable[[int], str]
type BadA91 = (lambda: int)()
type BadB91 = -1
type BadC91 = [int][0]
def use91(x: GoodA91, y: GoodB91) -> GoodC91 | None:
    return None

var92 = 1
type GoodA92 = int | None
type GoodB92 = dict[str, list[int]]
type GoodC92 = Callable[[int], str]
type BadA92 = [int, str]
type BadB92 = True
type BadC92 = 42
def use92(x: GoodA92, y: GoodB92) -> GoodC92 | None:
    return None

var93 = 1
type GoodA93 = int | None
type GoodB93 = dict[str, list[int]]
type GoodC93 = Callable[[int], str]
type BadA93 = {"a": "b"}
type BadB93 = var93
type BadC93 = int if 1 < 3 else str
def use93(x: GoodA93, y: GoodB93) -> GoodC93 | None:
    return None

var94 = 1
type GoodA94 = int | None
type GoodB94 = dict[str, list[int]]
type GoodC94 = Callable[[int], str]
type BadA94 = f"{'int'}"
type BadB94 = (int, str)
type BadC94 = list or set
def use94(x: GoodA94, y: GoodB94) -> GoodC94 | None:
    return None

var95 = 1
type GoodA95 = int | None
type GoodB95 = dict[str, list[int]]
type GoodC95 = Callable[[int], str]
type BadA95 = (lambda: int)()
type BadB95 = -1
type BadC95 = [int][0]
def use95(x: GoodA95, y: GoodB95) -> GoodC95 | None:
    return None

var96 = 1
type GoodA96 = int | None
type GoodB96 = dict[str, list[int]]
type GoodC96 = Callable[[int], str]
type BadA96 = [int, str]
type BadB96 = True
type BadC96 = 42
def use96(x: GoodA96, y: GoodB96) -> GoodC96 | None:
    return None

var97 = 1
type GoodA97 = int | None
type GoodB97 = dict[str, list[int]]
type GoodC97 = Callable[[int], str]
type BadA97 = {"a": "b"}
type BadB97 = var97
type BadC97 = int if 1 < 3 else str
def use97(x: GoodA97, y: GoodB97) -> GoodC97 | None:
    return None

var98 = 1
type GoodA98 = int | None
type GoodB98 = dict[str, list[int]]
type GoodC98 = Callable[[int], str]
type BadA98 = f"{'int'}"
type BadB98 = (int, str)
type BadC98 = list or set
def use98(x: GoodA98, y: GoodB98) -> GoodC98 | None:
    return None

var99 = 1
type GoodA99 = int | None
type GoodB99 = dict[str, list[int]]
type GoodC99 = Callable[[int], str]
type BadA99 = (lambda: int)()
type BadB99 = -1
type BadC99 = [int][0]
def use99(x: GoodA99, y: GoodB99) -> GoodC99 | None:
    return None

var100 = 1
type GoodA100 = int | None
type GoodB100 = dict[str, list[int]]
type GoodC100 = Callable[[int], str]
type BadA100 = [int, str]
type BadB100 = True
type BadC100 = 42
def use100(x: GoodA100, y: GoodB100) -> GoodC100 | None:
    return None

var101 = 1
type GoodA101 = int | None
type GoodB101 = dict[str, list[int]]
type GoodC101 = Callable[[int], str]
type BadA101 = {"a": "b"}
type BadB101 = var101
type BadC101 = int if 1 < 3 else str
def use101(x: GoodA101, y: GoodB101) -> GoodC101 | None:
    return None

var102 = 1
type GoodA102 = int | None
type GoodB102 = dict[str, list[int]]
type GoodC102 = Callable[[int], str]
type BadA102 = f"{'int'}"
type BadB102 = (int, str)
type BadC102 = list or set
def use102(x: GoodA102, y: GoodB102) -> GoodC102 | None:
    return None

var103 = 1
type GoodA103 = int | None
type GoodB103 = dict[str, list[int]]
type GoodC103 = Callable[[int], str]
type BadA103 = (lambda: int)()
type BadB103 = -1
type BadC103 = [int][0]
def use103(x: GoodA103, y: GoodB103) -> GoodC103 | None:
    return None

var104 = 1
type GoodA104 = int | None
type GoodB104 = dict[str, list[int]]
type GoodC104 = Callable[[int], str]
type BadA104 = [int, str]
type BadB104 = True
type BadC104 = 42
def use104(x: GoodA104, y: GoodB104) -> GoodC104 | None:
    return None

var105 = 1
type GoodA105 = int | None
type GoodB105 = dict[str, list[int]]
type GoodC105 = Callable[[int], str]
type BadA105 = {"a": "b"}
type BadB105 = var105
type BadC105 = int if 1 < 3 else str
def use105(x: GoodA105, y: GoodB105) -> GoodC105 | None:
    return None

var106 = 1
type GoodA106 = int | None
type GoodB106 = dict[str, list[int]]
type GoodC106 = Callable[[int], str]
type BadA106 = f"{'int'}"
type BadB106 = (int, str)
type BadC106 = list or set
def use106(x: GoodA106, y: GoodB106) -> GoodC106 | None:
    return None

var107 = 1
type GoodA107 = int | None
type GoodB107 = dict[str, list[int]]
type GoodC107 = Callable[[int], str]
type BadA107 = (lambda: int)()
type BadB107 = -1
type BadC107 = [int][0]
def use107(x: GoodA107, y: GoodB107) -> GoodC107 | None:
    return None

var108 = 1
type GoodA108 = int | None
type GoodB108 = dict[str, list[int]]
type GoodC108 = Callable[[int], str]
type BadA108 = [int, str]
type BadB108 = True
type BadC108 = 42
def use108(x: GoodA108, y: GoodB108) -> GoodC108 | None:
    return None

var109 = 1
type GoodA109 = int | None
type GoodB109 = dict[str, list[int]]
type GoodC109 = Callable[[int], str]
type BadA109 = {"a": "b"}
type BadB109 = var109
type BadC109 = int if 1 < 3 else str
def use109(x: GoodA109, y: GoodB109) -> GoodC109 | None:
    return None

var110 = 1
type GoodA110 = int | None
type GoodB110 = dict[str, list[int]]
type GoodC110 = Callable[[int], str]
type BadA110 = f"{'int'}"
type BadB110 = (int, str)
type BadC110 = list or set
def use110(x: GoodA110, y: GoodB110) -> GoodC110 | None:
    return None

var111 = 1
type GoodA111 = int | None
type GoodB111 = dict[str, list[int]]
type GoodC111 = Callable[[int], str]
type BadA111 = (lambda: int)()
type BadB111 = -1
type BadC111 = [int][0]
def use111(x: GoodA111, y: GoodB111) -> GoodC111 | None:
    return None

var112 = 1
type GoodA112 = int | None
type GoodB112 = dict[str, list[int]]
type GoodC112 = Callable[[int], str]
type BadA112 = [int, str]
type BadB112 = True
type BadC112 = 42
def use112(x: GoodA112, y: GoodB112) -> GoodC112 | None:
    return None

var113 = 1
type GoodA113 = int | None
type GoodB113 = dict[str, list[int]]
type GoodC113 = Callable[[int], str]
type BadA113 = {"a": "b"}
type BadB113 = var113
type BadC113 = int if 1 < 3 else str
def use113(x: GoodA113, y: GoodB113) -> GoodC113 | None:
    return None

var114 = 1
type GoodA114 = int | None
type GoodB114 = dict[str, list[int]]
type GoodC114 = Callable[[int], str]
type BadA114 = f"{'int'}"
type BadB114 = (int, str)
type BadC114 = list or set
def use114(x: GoodA114, y: GoodB114) -> GoodC114 | None:
    return None

var115 = 1
type GoodA115 = int | None
type GoodB115 = dict[str, list[int]]
type GoodC115 = Callable[[int], str]
type BadA115 = (lambda: int)()
type BadB115 = -1
type BadC115 = [int][0]
def use115(x: GoodA115, y: GoodB115) -> GoodC115 | None:
    return None

var116 = 1
type GoodA116 = int | None
type GoodB116 = dict[str, list[int]]
type GoodC116 = Callable[[int], str]
type BadA116 = [int, str]
type BadB116 = True
type BadC116 = 42
def use116(x: GoodA116, y: GoodB116) -> GoodC116 | None:
    return None

var117 = 1
type GoodA117 = int | None
type GoodB117 = dict[str, list[int]]
type GoodC117 = Callable[[int], str]
type BadA117 = {"a": "b"}
type BadB117 = var117
type BadC117 = int if 1 < 3 else str
def use117(x: GoodA117, y: GoodB117) -> GoodC117 | None:
    return None

var118 = 1
type GoodA118 = int | None
type GoodB118 = dict[str, list[int]]
type GoodC118 = Callable[[int], str]
type BadA118 = f"{'int'}"
type BadB118 = (int, str)
type BadC118 = list or set
def use118(x: GoodA118, y: GoodB118) -> GoodC118 | None:
    return None

var119 = 1
type GoodA119 = int | None
type GoodB119 = dict[str, list[int]]
type GoodC119 = Callable[[int], str]
type BadA119 = (lambda: int)()
type BadB119 = -1
type BadC119 = [int][0]
def use119(x: GoodA119, y: GoodB119) -> GoodC119 | None:
    return None

var120 = 1
type GoodA120 = int | None
type GoodB120 = dict[str, list[int]]
type GoodC120 = Callable[[int], str]
type BadA120 = [int, str]
type BadB120 = True
type BadC120 = 42
def use120(x: GoodA120, y: GoodB120) -> GoodC120 | None:
    return None

var121 = 1
type GoodA121 = int | None
type GoodB121 = dict[str, list[int]]
type GoodC121 = Callable[[int], str]
type BadA121 = {"a": "b"}
type BadB121 = var121
type BadC121 = int if 1 < 3 else str
def use121(x: GoodA121, y: GoodB121) -> GoodC121 | None:
    return None

var122 = 1
type GoodA122 = int | None
type GoodB122 = dict[str, list[int]]
type GoodC122 = Callable[[int], str]
type BadA122 = f"{'int'}"
type BadB122 = (int, str)
type BadC122 = list or set
def use122(x: GoodA122, y: GoodB122) -> GoodC122 | None:
    return None

var123 = 1
type GoodA123 = int | None
type GoodB123 = dict[str, list[int]]
type GoodC123 = Callable[[int], str]
type BadA123 = (lambda: int)()
type BadB123 = -1
type BadC123 = [int][0]
def use123(x: GoodA123, y: GoodB123) -> GoodC123 | None:
    return None

var124 = 1
type GoodA124 = int | None
type GoodB124 = dict[str, list[int]]
type GoodC124 = Callable[[int], str]
type BadA124 = [int, str]
type BadB124 = True
type BadC124 = 42
def use124(x: GoodA124, y: GoodB124) -> GoodC124 | None:
    return None

var125 = 1
type GoodA125 = int | None
type GoodB125 = dict[str, list[int]]
type GoodC125 = Callable[[int], str]
type BadA125 = {"a": "b"}
type BadB125 = var125
type BadC125 = int if 1 < 3 else str
def use125(x: GoodA125, y: GoodB125) -> GoodC125 | None:
    return None

var126 = 1
type GoodA126 = int | None
type GoodB126 = dict[str, list[int]]
type GoodC126 = Callable[[int], str]
type BadA126 = f"{'int'}"
type BadB126 = (int, str)
type BadC126 = list or set
def use126(x: GoodA126, y: GoodB126) -> GoodC126 | None:
    return None

var127 = 1
type GoodA127 = int | None
type GoodB127 = dict[str, list[int]]
type GoodC127 = Callable[[int], str]
type BadA127 = (lambda: int)()
type BadB127 = -1
type BadC127 = [int][0]
def use127(x: GoodA127, y: GoodB127) -> GoodC127 | None:
    return None

var128 = 1
type GoodA128 = int | None
type GoodB128 = dict[str, list[int]]
type GoodC128 = Callable[[int], str]
type BadA128 = [int, str]
type BadB128 = True
type BadC128 = 42
def use128(x: GoodA128, y: GoodB128) -> GoodC128 | None:
    return None

var129 = 1
type GoodA129 = int | None
type GoodB129 = dict[str, list[int]]
type GoodC129 = Callable[[int], str]
type BadA129 = {"a": "b"}
type BadB129 = var129
type BadC129 = int if 1 < 3 else str
def use129(x: GoodA129, y: GoodB129) -> GoodC129 | None:
    return None

var130 = 1
type GoodA130 = int | None
type GoodB130 = dict[str, list[int]]
type GoodC130 = Callable[[int], str]
type BadA130 = f"{'int'}"
type BadB130 = (int, str)
type BadC130 = list or set
def use130(x: GoodA130, y: GoodB130) -> GoodC130 | None:
    return None

var131 = 1
type GoodA131 = int | None
type GoodB131 = dict[str, list[int]]
type GoodC131 = Callable[[int], str]
type BadA131 = (lambda: int)()
type BadB131 = -1
type BadC131 = [int][0]
def use131(x: GoodA131, y: GoodB131) -> GoodC131 | None:
    return None

var132 = 1
type GoodA132 = int | None
type GoodB132 = dict[str, list[int]]
type GoodC132 = Callable[[int], str]
type BadA132 = [int, str]
type BadB132 = True
type BadC132 = 42
def use132(x: GoodA132, y: GoodB132) -> GoodC132 | None:
    return None

var133 = 1
type GoodA133 = int | None
type GoodB133 = dict[str, list[int]]
type GoodC133 = Callable[[int], str]
type BadA133 = {"a": "b"}
type BadB133 = var133
type BadC133 = int if 1 < 3 else str
def use133(x: GoodA133, y: GoodB133) -> GoodC133 | None:
    return None

var134 = 1
type GoodA134 = int | None
type GoodB134 = dict[str, list[int]]
type GoodC134 = Callable[[int], str]
type BadA134 = f"{'int'}"
type BadB134 = (int, str)
type BadC134 = list or set
def use134(x: GoodA134, y: GoodB134) -> GoodC134 | None:
    return None

var135 = 1
type GoodA135 = int | None
type GoodB135 = dict[str, list[int]]
type GoodC135 = Callable[[int], str]
type BadA135 = (lambda: int)()
type BadB135 = -1
type BadC135 = [int][0]
def use135(x: GoodA135, y: GoodB135) -> GoodC135 | None:
    return None

var136 = 1
type GoodA136 = int | None
type GoodB136 = dict[str, list[int]]
type GoodC136 = Callable[[int], str]
type BadA136 = [int, str]
type BadB136 = True
type BadC136 = 42
def use136(x: GoodA136, y: GoodB136) -> GoodC136 | None:
    return None

var137 = 1
type GoodA137 = int | None
type GoodB137 = dict[str, list[int]]
type GoodC137 = Callable[[int], str]
type BadA137 = {"a": "b"}
type BadB137 = var137
type BadC137 = int if 1 < 3 else str
def use137(x: GoodA137, y: GoodB137) -> GoodC137 | None:
    return None

var138 = 1
type GoodA138 = int | None
type GoodB138 = dict[str, list[int]]
type GoodC138 = Callable[[int], str]
type BadA138 = f"{'int'}"
type BadB138 = (int, str)
type BadC138 = list or set
def use138(x: GoodA138, y: GoodB138) -> GoodC138 | None:
    return None

var139 = 1
type GoodA139 = int | None
type GoodB139 = dict[str, list[int]]
type GoodC139 = Callable[[int], str]
type BadA139 = (lambda: int)()
type BadB139 = -1
type BadC139 = [int][0]
def use139(x: GoodA139, y: GoodB139) -> GoodC139 | None:
    return None

var140 = 1
type GoodA140 = int | None
type GoodB140 = dict[str, list[int]]
type GoodC140 = Callable[[int], str]
type BadA140 = [int, str]
type BadB140 = True
type BadC140 = 42
def use140(x: GoodA140, y: GoodB140) -> GoodC140 | None:
    return None

var141 = 1
type GoodA141 = int | None
type GoodB141 = dict[str, list[int]]
type GoodC141 = Callable[[int], str]
type BadA141 = {"a": "b"}
type BadB141 = var141
type BadC141 = int if 1 < 3 else str
def use141(x: GoodA141, y: GoodB141) -> GoodC141 | None:
    return None

var142 = 1
type GoodA142 = int | None
type GoodB142 = dict[str, list[int]]
type GoodC142 = Callable[[int], str]
type BadA142 = f"{'int'}"
type BadB142 = (int, str)
type BadC142 = list or set
def use142(x: GoodA142, y: GoodB142) -> GoodC142 | None:
    return None

var143 = 1
type GoodA143 = int | None
type GoodB143 = dict[str, list[int]]
type GoodC143 = Callable[[int], str]
type BadA143 = (lambda: int)()
type BadB143 = -1
type BadC143 = [int][0]
def use143(x: GoodA143, y: GoodB143) -> GoodC143 | None:
    return None

var144 = 1
type GoodA144 = int | None
type GoodB144 = dict[str, list[int]]
type GoodC144 = Callable[[int], str]
type BadA144 = [int, str]
type BadB144 = True
type BadC144 = 42
def use144(x: GoodA144, y: GoodB144) -> GoodC144 | None:
    return None

var145 = 1
type GoodA145 = int | None
type GoodB145 = dict[str, list[int]]
type GoodC145 = Callable[[int], str]
type BadA145 = {"a": "b"}
type BadB145 = var145
type BadC145 = int if 1 < 3 else str
def use145(x: GoodA145, y: GoodB145) -> GoodC145 | None:
    return None

var146 = 1
type GoodA146 = int | None
type GoodB146 = dict[str, list[int]]
type GoodC146 = Callable[[int], str]
type BadA146 = f"{'int'}"
type BadB146 = (int, str)
type BadC146 = list or set
def use146(x: GoodA146, y: GoodB146) -> GoodC146 | None:
    return None

var147 = 1
type GoodA147 = int | None
type GoodB147 = dict[str, list[int]]
type GoodC147 = Callable[[int], str]
type BadA147 = (lambda: int)()
type BadB147 = -1
type BadC147 = [int][0]
def use147(x: GoodA147, y: GoodB147) -> GoodC147 | None:
    return None

var148 = 1
type GoodA148 = int | None
type GoodB148 = dict[str, list[int]]
type GoodC148 = Callable[[int], str]
type BadA148 = [int, str]
type BadB148 = True
type BadC148 = 42
def use148(x: GoodA148, y: GoodB148) -> GoodC148 | None:
    return None

var149 = 1
type GoodA149 = int | None
type GoodB149 = dict[str, list[int]]
type GoodC149 = Callable[[int], str]
type BadA149 = {"a": "b"}
type BadB149 = var149
type BadC149 = int if 1 < 3 else str
def use149(x: GoodA149, y: GoodB149) -> GoodC149 | None:
    return None

var150 = 1
type GoodA150 = int | None
type GoodB150 = dict[str, list[int]]
type GoodC150 = Callable[[int], str]
type BadA150 = f"{'int'}"
type BadB150 = (int, str)
type BadC150 = list or set
def use150(x: GoodA150, y: GoodB150) -> GoodC150 | None:
    return None

var151 = 1
type GoodA151 = int | None
type GoodB151 = dict[str, list[int]]
type GoodC151 = Callable[[int], str]
type BadA151 = (lambda: int)()
type BadB151 = -1
type BadC151 = [int][0]
def use151(x: GoodA151, y: GoodB151) -> GoodC151 | None:
    return None

var152 = 1
type GoodA152 = int | None
type GoodB152 = dict[str, list[int]]
type GoodC152 = Callable[[int], str]
type BadA152 = [int, str]
type BadB152 = True
type BadC152 = 42
def use152(x: GoodA152, y: GoodB152) -> GoodC152 | None:
    return None

var153 = 1
type GoodA153 = int | None
type GoodB153 = dict[str, list[int]]
type GoodC153 = Callable[[int], str]
type BadA153 = {"a": "b"}
type BadB153 = var153
type BadC153 = int if 1 < 3 else str
def use153(x: GoodA153, y: GoodB153) -> GoodC153 | None:
    return None

var154 = 1
type GoodA154 = int | None
type GoodB154 = dict[str, list[int]]
type GoodC154 = Callable[[int], str]
type BadA154 = f"{'int'}"
type BadB154 = (int, str)
type BadC154 = list or set
def use154(x: GoodA154, y: GoodB154) -> GoodC154 | None:
    return None

var155 = 1
type GoodA155 = int | None
type GoodB155 = dict[str, list[int]]
type GoodC155 = Callable[[int], str]
type BadA155 = (lambda: int)()
type BadB155 = -1
type BadC155 = [int][0]
def use155(x: GoodA155, y: GoodB155) -> GoodC155 | None:
    return None

var156 = 1
type GoodA156 = int | None
type GoodB156 = dict[str, list[int]]
type GoodC156 = Callable[[int], str]
type BadA156 = [int, str]
type BadB156 = True
type BadC156 = 42
def use156(x: GoodA156, y: GoodB156) -> GoodC156 | None:
    return None

var157 = 1
type GoodA157 = int | None
type GoodB157 = dict[str, list[int]]
type GoodC157 = Callable[[int], str]
type BadA157 = {"a": "b"}
type BadB157 = var157
type BadC157 = int if 1 < 3 else str
def use157(x: GoodA157, y: GoodB157) -> GoodC157 | None:
    return None

var158 = 1
type GoodA158 = int | None
type GoodB158 = dict[str, list[int]]
type GoodC158 = Callable[[int], str]
type BadA158 = f"{'int'}"
type BadB158 = (int, str)
type BadC158 = list or set
def use158(x: GoodA158, y: GoodB158) -> GoodC158 | None:
    return None

var159 = 1
type GoodA159 = int | None
type GoodB159 = dict[str, list[int]]
type GoodC159 = Callable[[int], str]
type BadA159 = (lambda: int)()
type BadB159 = -1
type BadC159 = [int][0]
def use159(x: GoodA159, y: GoodB159) -> GoodC159 | None:
    return None

var160 = 1
type GoodA160 = int | None
type GoodB160 = dict[str, list[int]]
type GoodC160 = Callable[[int], str]
type BadA160 = [int, str]
type BadB160 = True
type BadC160 = 42
def use160(x: GoodA160, y: GoodB160) -> GoodC160 | None:
    return None

var161 = 1
type GoodA161 = int | None
type GoodB161 = dict[str, list[int]]
type GoodC161 = Callable[[int], str]
type BadA161 = {"a": "b"}
type BadB161 = var161
type BadC161 = int if 1 < 3 else str
def use161(x: GoodA161, y: GoodB161) -> GoodC161 | None:
    return None

var162 = 1
type GoodA162 = int | None
type GoodB162 = dict[str, list[int]]
type GoodC162 = Callable[[int], str]
type BadA162 = f"{'int'}"
type BadB162 = (int, str)
type BadC162 = list or set
def use162(x: GoodA162, y: GoodB162) -> GoodC162 | None:
    return None

var163 = 1
type GoodA163 = int | None
type GoodB163 = dict[str, list[int]]
type GoodC163 = Callable[[int], str]
type BadA163 = (lambda: int)()
type BadB163 = -1
type BadC163 = [int][0]
def use163(x: GoodA163, y: GoodB163) -> GoodC163 | None:
    return None

var164 = 1
type GoodA164 = int | None
type GoodB164 = dict[str, list[int]]
type GoodC164 = Callable[[int], str]
type BadA164 = [int, str]
type BadB164 = True
type BadC164 = 42
def use164(x: GoodA164, y: GoodB164) -> GoodC164 | None:
    return None

var165 = 1
type GoodA165 = int | None
type GoodB165 = dict[str, list[int]]
type GoodC165 = Callable[[int], str]
type BadA165 = {"a": "b"}
type BadB165 = var165
type BadC165 = int if 1 < 3 else str
def use165(x: GoodA165, y: GoodB165) -> GoodC165 | None:
    return None

var166 = 1
type GoodA166 = int | None
type GoodB166 = dict[str, list[int]]
type GoodC166 = Callable[[int], str]
type BadA166 = f"{'int'}"
type BadB166 = (int, str)
type BadC166 = list or set
def use166(x: GoodA166, y: GoodB166) -> GoodC166 | None:
    return None

var167 = 1
type GoodA167 = int | None
type GoodB167 = dict[str, list[int]]
type GoodC167 = Callable[[int], str]
type BadA167 = (lambda: int)()
type BadB167 = -1
type BadC167 = [int][0]
def use167(x: GoodA167, y: GoodB167) -> GoodC167 | None:
    return None

var168 = 1
type GoodA168 = int | None
type GoodB168 = dict[str, list[int]]
type GoodC168 = Callable[[int], str]
type BadA168 = [int, str]
type BadB168 = True
type BadC168 = 42
def use168(x: GoodA168, y: GoodB168) -> GoodC168 | None:
    return None

var169 = 1
type GoodA169 = int | None
type GoodB169 = dict[str, list[int]]
type GoodC169 = Callable[[int], str]
type BadA169 = {"a": "b"}
type BadB169 = var169
type BadC169 = int if 1 < 3 else str
def use169(x: GoodA169, y: GoodB169) -> GoodC169 | None:
    return None

var170 = 1
type GoodA170 = int | None
type GoodB170 = dict[str, list[int]]
type GoodC170 = Callable[[int], str]
type BadA170 = f"{'int'}"
type BadB170 = (int, str)
type BadC170 = list or set
def use170(x: GoodA170, y: GoodB170) -> GoodC170 | None:
    return None

var171 = 1
type GoodA171 = int | None
type GoodB171 = dict[str, list[int]]
type GoodC171 = Callable[[int], str]
type BadA171 = (lambda: int)()
type BadB171 = -1
type BadC171 = [int][0]
def use171(x: GoodA171, y: GoodB171) -> GoodC171 | None:
    return None

var172 = 1
type GoodA172 = int | None
type GoodB172 = dict[str, list[int]]
type GoodC172 = Callable[[int], str]
type BadA172 = [int, str]
type BadB172 = True
type BadC172 = 42
def use172(x: GoodA172, y: GoodB172) -> GoodC172 | None:
    return None

var173 = 1
type GoodA173 = int | None
type GoodB173 = dict[str, list[int]]
type GoodC173 = Callable[[int], str]
type BadA173 = {"a": "b"}
type BadB173 = var173
type BadC173 = int if 1 < 3 else str
def use173(x: GoodA173, y: GoodB173) -> GoodC173 | None:
    return None

var174 = 1
type GoodA174 = int | None
type GoodB174 = dict[str, list[int]]
type GoodC174 = Callable[[int], str]
type BadA174 = f"{'int'}"
type BadB174 = (int, str)
type BadC174 = list or set
def use174(x: GoodA174, y: GoodB174) -> GoodC174 | None:
    return None

var175 = 1
type GoodA175 = int | None
type GoodB175 = dict[str, list[int]]
type GoodC175 = Callable[[int], str]
type BadA175 = (lambda: int)()
type BadB175 = -1
type BadC175 = [int][0]
def use175(x: GoodA175, y: GoodB175) -> GoodC175 | None:
    return None

var176 = 1
type GoodA176 = int | None
type GoodB176 = dict[str, list[int]]
type GoodC176 = Callable[[int], str]
type BadA176 = [int, str]
type BadB176 = True
type BadC176 = 42
def use176(x: GoodA176, y: GoodB176) -> GoodC176 | None:
    return None

var177 = 1
type GoodA177 = int | None
type GoodB177 = dict[str, list[int]]
type GoodC177 = Callable[[int], str]
type BadA177 = {"a": "b"}
type BadB177 = var177
type BadC177 = int if 1 < 3 else str
def use177(x: GoodA177, y: GoodB177) -> GoodC177 | None:
    return None

var178 = 1
type GoodA178 = int | None
type GoodB178 = dict[str, list[int]]
type GoodC178 = Callable[[int], str]
type BadA178 = f"{'int'}"
type BadB178 = (int, str)
type BadC178 = list or set
def use178(x: GoodA178, y: GoodB178) -> GoodC178 | None:
    return None

var179 = 1
type GoodA179 = int | None
type GoodB179 = dict[str, list[int]]
type GoodC179 = Callable[[int], str]
type BadA179 = (lambda: int)()
type BadB179 = -1
type BadC179 = [int][0]
def use179(x: GoodA179, y: GoodB179) -> GoodC179 | None:
    return None

var180 = 1
type GoodA180 = int | None
type GoodB180 = dict[str, list[int]]
type GoodC180 = Callable[[int], str]
type BadA180 = [int, str]
type BadB180 = True
type BadC180 = 42
def use180(x: GoodA180, y: GoodB180) -> GoodC180 | None:
    return None

var181 = 1
type GoodA181 = int | None
type GoodB181 = dict[str, list[int]]
type GoodC181 = Callable[[int], str]
type BadA181 = {"a": "b"}
type BadB181 = var181
type BadC181 = int if 1 < 3 else str
def use181(x: GoodA181, y: GoodB181) -> GoodC181 | None:
    return None

var182 = 1
type GoodA182 = int | None
type GoodB182 = dict[str, list[int]]
type GoodC182 = Callable[[int], str]
type BadA182 = f"{'int'}"
type BadB182 = (int, str)
type BadC182 = list or set
def use182(x: GoodA182, y: GoodB182) -> GoodC182 | None:
    return None

var183 = 1
type GoodA183 = int | None
type GoodB183 = dict[str, list[int]]
type GoodC183 = Callable[[int], str]
type BadA183 = (lambda: int)()
type BadB183 = -1
type BadC183 = [int][0]
def use183(x: GoodA183, y: GoodB183) -> GoodC183 | None:
    return None

var184 = 1
type GoodA184 = int | None
type GoodB184 = dict[str, list[int]]
type GoodC184 = Callable[[int], str]
type BadA184 = [int, str]
type BadB184 = True
type BadC184 = 42
def use184(x: GoodA184, y: GoodB184) -> GoodC184 | None:
    return None

var185 = 1
type GoodA185 = int | None
type GoodB185 = dict[str, list[int]]
type GoodC185 = Callable[[int], str]
type BadA185 = {"a": "b"}
type BadB185 = var185
type BadC185 = int if 1 < 3 else str
def use185(x: GoodA185, y: GoodB185) -> GoodC185 | None:
    return None

var186 = 1
type GoodA186 = int | None
type GoodB186 = dict[str, list[int]]
type GoodC186 = Callable[[int], str]
type BadA186 = f"{'int'}"
type BadB186 = (int, str)
type BadC186 = list or set
def use186(x: GoodA186, y: GoodB186) -> GoodC186 | None:
    return None

var187 = 1
type GoodA187 = int | None
type GoodB187 = dict[str, list[int]]
type GoodC187 = Callable[[int], str]
type BadA187 = (lambda: int)()
type BadB187 = -1
type BadC187 = [int][0]
def use187(x: GoodA187, y: GoodB187) -> GoodC187 | None:
    return None

var188 = 1
type GoodA188 = int | None
type GoodB188 = dict[str, list[int]]
type GoodC188 = Callable[[int], str]
type BadA188 = [int, str]
type BadB188 = True
type BadC188 = 42
def use188(x: GoodA188, y: GoodB188) -> GoodC188 | None:
    return None

var189 = 1
type GoodA189 = int | None
type GoodB189 = dict[str, list[int]]
type GoodC189 = Callable[[int], str]
type BadA189 = {"a": "b"}
type BadB189 = var189
type BadC189 = int if 1 < 3 else str
def use189(x: GoodA189, y: GoodB189) -> GoodC189 | None:
    return None

var190 = 1
type GoodA190 = int | None
type GoodB190 = dict[str, list[int]]
type GoodC190 = Callable[[int], str]
type BadA190 = f"{'int'}"
type BadB190 = (int, str)
type BadC190 = list or set
def use190(x: GoodA190, y: GoodB190) -> GoodC190 | None:
    return None

var191 = 1
type GoodA191 = int | None
type GoodB191 = dict[str, list[int]]
type GoodC191 = Callable[[int], str]
type BadA191 = (lambda: int)()
type BadB191 = -1
type BadC191 = [int][0]
def use191(x: GoodA191, y: GoodB191) -> GoodC191 | None:
    return None

var192 = 1
type GoodA192 = int | None
type GoodB192 = dict[str, list[int]]
type GoodC192 = Callable[[int], str]
type BadA192 = [int, str]
type BadB192 = True
type BadC192 = 42
def use192(x: GoodA192, y: GoodB192) -> GoodC192 | None:
    return None

var193 = 1
type GoodA193 = int | None
type GoodB193 = dict[str, list[int]]
type GoodC193 = Callable[[int], str]
type BadA193 = {"a": "b"}
type BadB193 = var193
type BadC193 = int if 1 < 3 else str
def use193(x: GoodA193, y: GoodB193) -> GoodC193 | None:
    return None

var194 = 1
type GoodA194 = int | None
type GoodB194 = dict[str, list[int]]
type GoodC194 = Callable[[int], str]
type BadA194 = f"{'int'}"
type BadB194 = (int, str)
type BadC194 = list or set
def use194(x: GoodA194, y: GoodB194) -> GoodC194 | None:
    return None

var195 = 1
type GoodA195 = int | None
type GoodB195 = dict[str, list[int]]
type GoodC195 = Callable[[int], str]
type BadA195 = (lambda: int)()
type BadB195 = -1
type BadC195 = [int][0]
def use195(x: GoodA195, y: GoodB195) -> GoodC195 | None:
    return None

var196 = 1
type GoodA196 = int | None
type GoodB196 = dict[str, list[int]]
type GoodC196 = Callable[[int], str]
type BadA196 = [int, str]
type BadB196 = True
type BadC196 = 42
def use196(x: GoodA196, y: GoodB196) -> GoodC196 | None:
    return None

var197 = 1
type GoodA197 = int | None
type GoodB197 = dict[str, list[int]]
type GoodC197 = Callable[[int], str]
type BadA197 = {"a": "b"}
type BadB197 = var197
type BadC197 = int if 1 < 3 else str
def use197(x: GoodA197, y: GoodB197) -> GoodC197 | None:
    return None

var198 = 1
type GoodA198 = int | None
type GoodB198 = dict[str, list[int]]
type GoodC198 = Callable[[int], str]
type BadA198 = f"{'int'}"
type BadB198 = (int, str)
type BadC198 = list or set
def use198(x: GoodA198, y: GoodB198) -> GoodC198 | None:
    return None

var199 = 1
type GoodA199 = int | None
type GoodB199 = dict[str, list[int]]
type GoodC199 = Callable[[int], str]
type BadA199 = (lambda: int)()
type BadB199 = -1
type BadC199 = [int][0]
def use199(x: GoodA199, y: GoodB199) -> GoodC199 | None:
    return None

