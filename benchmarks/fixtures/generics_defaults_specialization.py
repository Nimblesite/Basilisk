# Benchmark stress fixture for the `generics_defaults_specialization` rule.
# Repeats numbered blocks of 4 arity-error patterns: too few type arguments
# (defaulted params), too many type arguments (class and TypeAlias), and
# subscripting a fully-specialised subclass with no free type parameters.

from typing import Generic, TypeAlias
from typing_extensions import TypeVar

T1 = TypeVar("T1")
T2 = TypeVar("T2")
DefaultStrT = TypeVar("DefaultStrT", default=str)

# block 0: generic arity stress
class Multi0(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi0[int]
class Single0(Generic[T1]):
    item: T1
Single0[int, str]
Alias0: TypeAlias = Single0[T2]
Alias0[int, str]
class Sealed0(Single0[int]):
    pass
Sealed0[str]

# block 1: generic arity stress
class Multi1(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi1[int]
class Single1(Generic[T1]):
    item: T1
Single1[int, str]
Alias1: TypeAlias = Single1[T2]
Alias1[int, str]
class Sealed1(Single1[int]):
    pass
Sealed1[str]

# block 2: generic arity stress
class Multi2(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi2[int]
class Single2(Generic[T1]):
    item: T1
Single2[int, str]
Alias2: TypeAlias = Single2[T2]
Alias2[int, str]
class Sealed2(Single2[int]):
    pass
Sealed2[str]

# block 3: generic arity stress
class Multi3(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi3[int]
class Single3(Generic[T1]):
    item: T1
Single3[int, str]
Alias3: TypeAlias = Single3[T2]
Alias3[int, str]
class Sealed3(Single3[int]):
    pass
Sealed3[str]

# block 4: generic arity stress
class Multi4(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi4[int]
class Single4(Generic[T1]):
    item: T1
Single4[int, str]
Alias4: TypeAlias = Single4[T2]
Alias4[int, str]
class Sealed4(Single4[int]):
    pass
Sealed4[str]

# block 5: generic arity stress
class Multi5(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi5[int]
class Single5(Generic[T1]):
    item: T1
Single5[int, str]
Alias5: TypeAlias = Single5[T2]
Alias5[int, str]
class Sealed5(Single5[int]):
    pass
Sealed5[str]

# block 6: generic arity stress
class Multi6(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi6[int]
class Single6(Generic[T1]):
    item: T1
Single6[int, str]
Alias6: TypeAlias = Single6[T2]
Alias6[int, str]
class Sealed6(Single6[int]):
    pass
Sealed6[str]

# block 7: generic arity stress
class Multi7(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi7[int]
class Single7(Generic[T1]):
    item: T1
Single7[int, str]
Alias7: TypeAlias = Single7[T2]
Alias7[int, str]
class Sealed7(Single7[int]):
    pass
Sealed7[str]

# block 8: generic arity stress
class Multi8(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi8[int]
class Single8(Generic[T1]):
    item: T1
Single8[int, str]
Alias8: TypeAlias = Single8[T2]
Alias8[int, str]
class Sealed8(Single8[int]):
    pass
Sealed8[str]

# block 9: generic arity stress
class Multi9(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi9[int]
class Single9(Generic[T1]):
    item: T1
Single9[int, str]
Alias9: TypeAlias = Single9[T2]
Alias9[int, str]
class Sealed9(Single9[int]):
    pass
Sealed9[str]

# block 10: generic arity stress
class Multi10(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi10[int]
class Single10(Generic[T1]):
    item: T1
Single10[int, str]
Alias10: TypeAlias = Single10[T2]
Alias10[int, str]
class Sealed10(Single10[int]):
    pass
Sealed10[str]

# block 11: generic arity stress
class Multi11(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi11[int]
class Single11(Generic[T1]):
    item: T1
Single11[int, str]
Alias11: TypeAlias = Single11[T2]
Alias11[int, str]
class Sealed11(Single11[int]):
    pass
Sealed11[str]

# block 12: generic arity stress
class Multi12(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi12[int]
class Single12(Generic[T1]):
    item: T1
Single12[int, str]
Alias12: TypeAlias = Single12[T2]
Alias12[int, str]
class Sealed12(Single12[int]):
    pass
Sealed12[str]

# block 13: generic arity stress
class Multi13(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi13[int]
class Single13(Generic[T1]):
    item: T1
Single13[int, str]
Alias13: TypeAlias = Single13[T2]
Alias13[int, str]
class Sealed13(Single13[int]):
    pass
Sealed13[str]

# block 14: generic arity stress
class Multi14(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi14[int]
class Single14(Generic[T1]):
    item: T1
Single14[int, str]
Alias14: TypeAlias = Single14[T2]
Alias14[int, str]
class Sealed14(Single14[int]):
    pass
Sealed14[str]

# block 15: generic arity stress
class Multi15(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi15[int]
class Single15(Generic[T1]):
    item: T1
Single15[int, str]
Alias15: TypeAlias = Single15[T2]
Alias15[int, str]
class Sealed15(Single15[int]):
    pass
Sealed15[str]

# block 16: generic arity stress
class Multi16(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi16[int]
class Single16(Generic[T1]):
    item: T1
Single16[int, str]
Alias16: TypeAlias = Single16[T2]
Alias16[int, str]
class Sealed16(Single16[int]):
    pass
Sealed16[str]

# block 17: generic arity stress
class Multi17(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi17[int]
class Single17(Generic[T1]):
    item: T1
Single17[int, str]
Alias17: TypeAlias = Single17[T2]
Alias17[int, str]
class Sealed17(Single17[int]):
    pass
Sealed17[str]

# block 18: generic arity stress
class Multi18(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi18[int]
class Single18(Generic[T1]):
    item: T1
Single18[int, str]
Alias18: TypeAlias = Single18[T2]
Alias18[int, str]
class Sealed18(Single18[int]):
    pass
Sealed18[str]

# block 19: generic arity stress
class Multi19(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi19[int]
class Single19(Generic[T1]):
    item: T1
Single19[int, str]
Alias19: TypeAlias = Single19[T2]
Alias19[int, str]
class Sealed19(Single19[int]):
    pass
Sealed19[str]

# block 20: generic arity stress
class Multi20(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi20[int]
class Single20(Generic[T1]):
    item: T1
Single20[int, str]
Alias20: TypeAlias = Single20[T2]
Alias20[int, str]
class Sealed20(Single20[int]):
    pass
Sealed20[str]

# block 21: generic arity stress
class Multi21(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi21[int]
class Single21(Generic[T1]):
    item: T1
Single21[int, str]
Alias21: TypeAlias = Single21[T2]
Alias21[int, str]
class Sealed21(Single21[int]):
    pass
Sealed21[str]

# block 22: generic arity stress
class Multi22(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi22[int]
class Single22(Generic[T1]):
    item: T1
Single22[int, str]
Alias22: TypeAlias = Single22[T2]
Alias22[int, str]
class Sealed22(Single22[int]):
    pass
Sealed22[str]

# block 23: generic arity stress
class Multi23(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi23[int]
class Single23(Generic[T1]):
    item: T1
Single23[int, str]
Alias23: TypeAlias = Single23[T2]
Alias23[int, str]
class Sealed23(Single23[int]):
    pass
Sealed23[str]

# block 24: generic arity stress
class Multi24(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi24[int]
class Single24(Generic[T1]):
    item: T1
Single24[int, str]
Alias24: TypeAlias = Single24[T2]
Alias24[int, str]
class Sealed24(Single24[int]):
    pass
Sealed24[str]

# block 25: generic arity stress
class Multi25(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi25[int]
class Single25(Generic[T1]):
    item: T1
Single25[int, str]
Alias25: TypeAlias = Single25[T2]
Alias25[int, str]
class Sealed25(Single25[int]):
    pass
Sealed25[str]

# block 26: generic arity stress
class Multi26(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi26[int]
class Single26(Generic[T1]):
    item: T1
Single26[int, str]
Alias26: TypeAlias = Single26[T2]
Alias26[int, str]
class Sealed26(Single26[int]):
    pass
Sealed26[str]

# block 27: generic arity stress
class Multi27(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi27[int]
class Single27(Generic[T1]):
    item: T1
Single27[int, str]
Alias27: TypeAlias = Single27[T2]
Alias27[int, str]
class Sealed27(Single27[int]):
    pass
Sealed27[str]

# block 28: generic arity stress
class Multi28(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi28[int]
class Single28(Generic[T1]):
    item: T1
Single28[int, str]
Alias28: TypeAlias = Single28[T2]
Alias28[int, str]
class Sealed28(Single28[int]):
    pass
Sealed28[str]

# block 29: generic arity stress
class Multi29(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi29[int]
class Single29(Generic[T1]):
    item: T1
Single29[int, str]
Alias29: TypeAlias = Single29[T2]
Alias29[int, str]
class Sealed29(Single29[int]):
    pass
Sealed29[str]

# block 30: generic arity stress
class Multi30(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi30[int]
class Single30(Generic[T1]):
    item: T1
Single30[int, str]
Alias30: TypeAlias = Single30[T2]
Alias30[int, str]
class Sealed30(Single30[int]):
    pass
Sealed30[str]

# block 31: generic arity stress
class Multi31(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi31[int]
class Single31(Generic[T1]):
    item: T1
Single31[int, str]
Alias31: TypeAlias = Single31[T2]
Alias31[int, str]
class Sealed31(Single31[int]):
    pass
Sealed31[str]

# block 32: generic arity stress
class Multi32(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi32[int]
class Single32(Generic[T1]):
    item: T1
Single32[int, str]
Alias32: TypeAlias = Single32[T2]
Alias32[int, str]
class Sealed32(Single32[int]):
    pass
Sealed32[str]

# block 33: generic arity stress
class Multi33(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi33[int]
class Single33(Generic[T1]):
    item: T1
Single33[int, str]
Alias33: TypeAlias = Single33[T2]
Alias33[int, str]
class Sealed33(Single33[int]):
    pass
Sealed33[str]

# block 34: generic arity stress
class Multi34(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi34[int]
class Single34(Generic[T1]):
    item: T1
Single34[int, str]
Alias34: TypeAlias = Single34[T2]
Alias34[int, str]
class Sealed34(Single34[int]):
    pass
Sealed34[str]

# block 35: generic arity stress
class Multi35(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi35[int]
class Single35(Generic[T1]):
    item: T1
Single35[int, str]
Alias35: TypeAlias = Single35[T2]
Alias35[int, str]
class Sealed35(Single35[int]):
    pass
Sealed35[str]

# block 36: generic arity stress
class Multi36(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi36[int]
class Single36(Generic[T1]):
    item: T1
Single36[int, str]
Alias36: TypeAlias = Single36[T2]
Alias36[int, str]
class Sealed36(Single36[int]):
    pass
Sealed36[str]

# block 37: generic arity stress
class Multi37(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi37[int]
class Single37(Generic[T1]):
    item: T1
Single37[int, str]
Alias37: TypeAlias = Single37[T2]
Alias37[int, str]
class Sealed37(Single37[int]):
    pass
Sealed37[str]

# block 38: generic arity stress
class Multi38(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi38[int]
class Single38(Generic[T1]):
    item: T1
Single38[int, str]
Alias38: TypeAlias = Single38[T2]
Alias38[int, str]
class Sealed38(Single38[int]):
    pass
Sealed38[str]

# block 39: generic arity stress
class Multi39(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi39[int]
class Single39(Generic[T1]):
    item: T1
Single39[int, str]
Alias39: TypeAlias = Single39[T2]
Alias39[int, str]
class Sealed39(Single39[int]):
    pass
Sealed39[str]

# block 40: generic arity stress
class Multi40(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi40[int]
class Single40(Generic[T1]):
    item: T1
Single40[int, str]
Alias40: TypeAlias = Single40[T2]
Alias40[int, str]
class Sealed40(Single40[int]):
    pass
Sealed40[str]

# block 41: generic arity stress
class Multi41(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi41[int]
class Single41(Generic[T1]):
    item: T1
Single41[int, str]
Alias41: TypeAlias = Single41[T2]
Alias41[int, str]
class Sealed41(Single41[int]):
    pass
Sealed41[str]

# block 42: generic arity stress
class Multi42(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi42[int]
class Single42(Generic[T1]):
    item: T1
Single42[int, str]
Alias42: TypeAlias = Single42[T2]
Alias42[int, str]
class Sealed42(Single42[int]):
    pass
Sealed42[str]

# block 43: generic arity stress
class Multi43(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi43[int]
class Single43(Generic[T1]):
    item: T1
Single43[int, str]
Alias43: TypeAlias = Single43[T2]
Alias43[int, str]
class Sealed43(Single43[int]):
    pass
Sealed43[str]

# block 44: generic arity stress
class Multi44(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi44[int]
class Single44(Generic[T1]):
    item: T1
Single44[int, str]
Alias44: TypeAlias = Single44[T2]
Alias44[int, str]
class Sealed44(Single44[int]):
    pass
Sealed44[str]

# block 45: generic arity stress
class Multi45(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi45[int]
class Single45(Generic[T1]):
    item: T1
Single45[int, str]
Alias45: TypeAlias = Single45[T2]
Alias45[int, str]
class Sealed45(Single45[int]):
    pass
Sealed45[str]

# block 46: generic arity stress
class Multi46(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi46[int]
class Single46(Generic[T1]):
    item: T1
Single46[int, str]
Alias46: TypeAlias = Single46[T2]
Alias46[int, str]
class Sealed46(Single46[int]):
    pass
Sealed46[str]

# block 47: generic arity stress
class Multi47(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi47[int]
class Single47(Generic[T1]):
    item: T1
Single47[int, str]
Alias47: TypeAlias = Single47[T2]
Alias47[int, str]
class Sealed47(Single47[int]):
    pass
Sealed47[str]

# block 48: generic arity stress
class Multi48(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi48[int]
class Single48(Generic[T1]):
    item: T1
Single48[int, str]
Alias48: TypeAlias = Single48[T2]
Alias48[int, str]
class Sealed48(Single48[int]):
    pass
Sealed48[str]

# block 49: generic arity stress
class Multi49(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi49[int]
class Single49(Generic[T1]):
    item: T1
Single49[int, str]
Alias49: TypeAlias = Single49[T2]
Alias49[int, str]
class Sealed49(Single49[int]):
    pass
Sealed49[str]

# block 50: generic arity stress
class Multi50(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi50[int]
class Single50(Generic[T1]):
    item: T1
Single50[int, str]
Alias50: TypeAlias = Single50[T2]
Alias50[int, str]
class Sealed50(Single50[int]):
    pass
Sealed50[str]

# block 51: generic arity stress
class Multi51(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi51[int]
class Single51(Generic[T1]):
    item: T1
Single51[int, str]
Alias51: TypeAlias = Single51[T2]
Alias51[int, str]
class Sealed51(Single51[int]):
    pass
Sealed51[str]

# block 52: generic arity stress
class Multi52(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi52[int]
class Single52(Generic[T1]):
    item: T1
Single52[int, str]
Alias52: TypeAlias = Single52[T2]
Alias52[int, str]
class Sealed52(Single52[int]):
    pass
Sealed52[str]

# block 53: generic arity stress
class Multi53(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi53[int]
class Single53(Generic[T1]):
    item: T1
Single53[int, str]
Alias53: TypeAlias = Single53[T2]
Alias53[int, str]
class Sealed53(Single53[int]):
    pass
Sealed53[str]

# block 54: generic arity stress
class Multi54(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi54[int]
class Single54(Generic[T1]):
    item: T1
Single54[int, str]
Alias54: TypeAlias = Single54[T2]
Alias54[int, str]
class Sealed54(Single54[int]):
    pass
Sealed54[str]

# block 55: generic arity stress
class Multi55(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi55[int]
class Single55(Generic[T1]):
    item: T1
Single55[int, str]
Alias55: TypeAlias = Single55[T2]
Alias55[int, str]
class Sealed55(Single55[int]):
    pass
Sealed55[str]

# block 56: generic arity stress
class Multi56(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi56[int]
class Single56(Generic[T1]):
    item: T1
Single56[int, str]
Alias56: TypeAlias = Single56[T2]
Alias56[int, str]
class Sealed56(Single56[int]):
    pass
Sealed56[str]

# block 57: generic arity stress
class Multi57(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi57[int]
class Single57(Generic[T1]):
    item: T1
Single57[int, str]
Alias57: TypeAlias = Single57[T2]
Alias57[int, str]
class Sealed57(Single57[int]):
    pass
Sealed57[str]

# block 58: generic arity stress
class Multi58(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi58[int]
class Single58(Generic[T1]):
    item: T1
Single58[int, str]
Alias58: TypeAlias = Single58[T2]
Alias58[int, str]
class Sealed58(Single58[int]):
    pass
Sealed58[str]

# block 59: generic arity stress
class Multi59(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi59[int]
class Single59(Generic[T1]):
    item: T1
Single59[int, str]
Alias59: TypeAlias = Single59[T2]
Alias59[int, str]
class Sealed59(Single59[int]):
    pass
Sealed59[str]

# block 60: generic arity stress
class Multi60(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi60[int]
class Single60(Generic[T1]):
    item: T1
Single60[int, str]
Alias60: TypeAlias = Single60[T2]
Alias60[int, str]
class Sealed60(Single60[int]):
    pass
Sealed60[str]

# block 61: generic arity stress
class Multi61(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi61[int]
class Single61(Generic[T1]):
    item: T1
Single61[int, str]
Alias61: TypeAlias = Single61[T2]
Alias61[int, str]
class Sealed61(Single61[int]):
    pass
Sealed61[str]

# block 62: generic arity stress
class Multi62(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi62[int]
class Single62(Generic[T1]):
    item: T1
Single62[int, str]
Alias62: TypeAlias = Single62[T2]
Alias62[int, str]
class Sealed62(Single62[int]):
    pass
Sealed62[str]

# block 63: generic arity stress
class Multi63(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi63[int]
class Single63(Generic[T1]):
    item: T1
Single63[int, str]
Alias63: TypeAlias = Single63[T2]
Alias63[int, str]
class Sealed63(Single63[int]):
    pass
Sealed63[str]

# block 64: generic arity stress
class Multi64(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi64[int]
class Single64(Generic[T1]):
    item: T1
Single64[int, str]
Alias64: TypeAlias = Single64[T2]
Alias64[int, str]
class Sealed64(Single64[int]):
    pass
Sealed64[str]

# block 65: generic arity stress
class Multi65(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi65[int]
class Single65(Generic[T1]):
    item: T1
Single65[int, str]
Alias65: TypeAlias = Single65[T2]
Alias65[int, str]
class Sealed65(Single65[int]):
    pass
Sealed65[str]

# block 66: generic arity stress
class Multi66(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi66[int]
class Single66(Generic[T1]):
    item: T1
Single66[int, str]
Alias66: TypeAlias = Single66[T2]
Alias66[int, str]
class Sealed66(Single66[int]):
    pass
Sealed66[str]

# block 67: generic arity stress
class Multi67(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi67[int]
class Single67(Generic[T1]):
    item: T1
Single67[int, str]
Alias67: TypeAlias = Single67[T2]
Alias67[int, str]
class Sealed67(Single67[int]):
    pass
Sealed67[str]

# block 68: generic arity stress
class Multi68(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi68[int]
class Single68(Generic[T1]):
    item: T1
Single68[int, str]
Alias68: TypeAlias = Single68[T2]
Alias68[int, str]
class Sealed68(Single68[int]):
    pass
Sealed68[str]

# block 69: generic arity stress
class Multi69(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi69[int]
class Single69(Generic[T1]):
    item: T1
Single69[int, str]
Alias69: TypeAlias = Single69[T2]
Alias69[int, str]
class Sealed69(Single69[int]):
    pass
Sealed69[str]

# block 70: generic arity stress
class Multi70(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi70[int]
class Single70(Generic[T1]):
    item: T1
Single70[int, str]
Alias70: TypeAlias = Single70[T2]
Alias70[int, str]
class Sealed70(Single70[int]):
    pass
Sealed70[str]

# block 71: generic arity stress
class Multi71(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi71[int]
class Single71(Generic[T1]):
    item: T1
Single71[int, str]
Alias71: TypeAlias = Single71[T2]
Alias71[int, str]
class Sealed71(Single71[int]):
    pass
Sealed71[str]

# block 72: generic arity stress
class Multi72(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi72[int]
class Single72(Generic[T1]):
    item: T1
Single72[int, str]
Alias72: TypeAlias = Single72[T2]
Alias72[int, str]
class Sealed72(Single72[int]):
    pass
Sealed72[str]

# block 73: generic arity stress
class Multi73(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi73[int]
class Single73(Generic[T1]):
    item: T1
Single73[int, str]
Alias73: TypeAlias = Single73[T2]
Alias73[int, str]
class Sealed73(Single73[int]):
    pass
Sealed73[str]

# block 74: generic arity stress
class Multi74(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi74[int]
class Single74(Generic[T1]):
    item: T1
Single74[int, str]
Alias74: TypeAlias = Single74[T2]
Alias74[int, str]
class Sealed74(Single74[int]):
    pass
Sealed74[str]

# block 75: generic arity stress
class Multi75(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi75[int]
class Single75(Generic[T1]):
    item: T1
Single75[int, str]
Alias75: TypeAlias = Single75[T2]
Alias75[int, str]
class Sealed75(Single75[int]):
    pass
Sealed75[str]

# block 76: generic arity stress
class Multi76(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi76[int]
class Single76(Generic[T1]):
    item: T1
Single76[int, str]
Alias76: TypeAlias = Single76[T2]
Alias76[int, str]
class Sealed76(Single76[int]):
    pass
Sealed76[str]

# block 77: generic arity stress
class Multi77(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi77[int]
class Single77(Generic[T1]):
    item: T1
Single77[int, str]
Alias77: TypeAlias = Single77[T2]
Alias77[int, str]
class Sealed77(Single77[int]):
    pass
Sealed77[str]

# block 78: generic arity stress
class Multi78(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi78[int]
class Single78(Generic[T1]):
    item: T1
Single78[int, str]
Alias78: TypeAlias = Single78[T2]
Alias78[int, str]
class Sealed78(Single78[int]):
    pass
Sealed78[str]

# block 79: generic arity stress
class Multi79(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi79[int]
class Single79(Generic[T1]):
    item: T1
Single79[int, str]
Alias79: TypeAlias = Single79[T2]
Alias79[int, str]
class Sealed79(Single79[int]):
    pass
Sealed79[str]

# block 80: generic arity stress
class Multi80(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi80[int]
class Single80(Generic[T1]):
    item: T1
Single80[int, str]
Alias80: TypeAlias = Single80[T2]
Alias80[int, str]
class Sealed80(Single80[int]):
    pass
Sealed80[str]

# block 81: generic arity stress
class Multi81(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi81[int]
class Single81(Generic[T1]):
    item: T1
Single81[int, str]
Alias81: TypeAlias = Single81[T2]
Alias81[int, str]
class Sealed81(Single81[int]):
    pass
Sealed81[str]

# block 82: generic arity stress
class Multi82(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi82[int]
class Single82(Generic[T1]):
    item: T1
Single82[int, str]
Alias82: TypeAlias = Single82[T2]
Alias82[int, str]
class Sealed82(Single82[int]):
    pass
Sealed82[str]

# block 83: generic arity stress
class Multi83(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi83[int]
class Single83(Generic[T1]):
    item: T1
Single83[int, str]
Alias83: TypeAlias = Single83[T2]
Alias83[int, str]
class Sealed83(Single83[int]):
    pass
Sealed83[str]

# block 84: generic arity stress
class Multi84(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi84[int]
class Single84(Generic[T1]):
    item: T1
Single84[int, str]
Alias84: TypeAlias = Single84[T2]
Alias84[int, str]
class Sealed84(Single84[int]):
    pass
Sealed84[str]

# block 85: generic arity stress
class Multi85(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi85[int]
class Single85(Generic[T1]):
    item: T1
Single85[int, str]
Alias85: TypeAlias = Single85[T2]
Alias85[int, str]
class Sealed85(Single85[int]):
    pass
Sealed85[str]

# block 86: generic arity stress
class Multi86(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi86[int]
class Single86(Generic[T1]):
    item: T1
Single86[int, str]
Alias86: TypeAlias = Single86[T2]
Alias86[int, str]
class Sealed86(Single86[int]):
    pass
Sealed86[str]

# block 87: generic arity stress
class Multi87(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi87[int]
class Single87(Generic[T1]):
    item: T1
Single87[int, str]
Alias87: TypeAlias = Single87[T2]
Alias87[int, str]
class Sealed87(Single87[int]):
    pass
Sealed87[str]

# block 88: generic arity stress
class Multi88(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi88[int]
class Single88(Generic[T1]):
    item: T1
Single88[int, str]
Alias88: TypeAlias = Single88[T2]
Alias88[int, str]
class Sealed88(Single88[int]):
    pass
Sealed88[str]

# block 89: generic arity stress
class Multi89(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi89[int]
class Single89(Generic[T1]):
    item: T1
Single89[int, str]
Alias89: TypeAlias = Single89[T2]
Alias89[int, str]
class Sealed89(Single89[int]):
    pass
Sealed89[str]

# block 90: generic arity stress
class Multi90(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi90[int]
class Single90(Generic[T1]):
    item: T1
Single90[int, str]
Alias90: TypeAlias = Single90[T2]
Alias90[int, str]
class Sealed90(Single90[int]):
    pass
Sealed90[str]

# block 91: generic arity stress
class Multi91(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi91[int]
class Single91(Generic[T1]):
    item: T1
Single91[int, str]
Alias91: TypeAlias = Single91[T2]
Alias91[int, str]
class Sealed91(Single91[int]):
    pass
Sealed91[str]

# block 92: generic arity stress
class Multi92(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi92[int]
class Single92(Generic[T1]):
    item: T1
Single92[int, str]
Alias92: TypeAlias = Single92[T2]
Alias92[int, str]
class Sealed92(Single92[int]):
    pass
Sealed92[str]

# block 93: generic arity stress
class Multi93(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi93[int]
class Single93(Generic[T1]):
    item: T1
Single93[int, str]
Alias93: TypeAlias = Single93[T2]
Alias93[int, str]
class Sealed93(Single93[int]):
    pass
Sealed93[str]

# block 94: generic arity stress
class Multi94(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi94[int]
class Single94(Generic[T1]):
    item: T1
Single94[int, str]
Alias94: TypeAlias = Single94[T2]
Alias94[int, str]
class Sealed94(Single94[int]):
    pass
Sealed94[str]

# block 95: generic arity stress
class Multi95(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi95[int]
class Single95(Generic[T1]):
    item: T1
Single95[int, str]
Alias95: TypeAlias = Single95[T2]
Alias95[int, str]
class Sealed95(Single95[int]):
    pass
Sealed95[str]

# block 96: generic arity stress
class Multi96(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi96[int]
class Single96(Generic[T1]):
    item: T1
Single96[int, str]
Alias96: TypeAlias = Single96[T2]
Alias96[int, str]
class Sealed96(Single96[int]):
    pass
Sealed96[str]

# block 97: generic arity stress
class Multi97(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi97[int]
class Single97(Generic[T1]):
    item: T1
Single97[int, str]
Alias97: TypeAlias = Single97[T2]
Alias97[int, str]
class Sealed97(Single97[int]):
    pass
Sealed97[str]

# block 98: generic arity stress
class Multi98(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi98[int]
class Single98(Generic[T1]):
    item: T1
Single98[int, str]
Alias98: TypeAlias = Single98[T2]
Alias98[int, str]
class Sealed98(Single98[int]):
    pass
Sealed98[str]

# block 99: generic arity stress
class Multi99(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi99[int]
class Single99(Generic[T1]):
    item: T1
Single99[int, str]
Alias99: TypeAlias = Single99[T2]
Alias99[int, str]
class Sealed99(Single99[int]):
    pass
Sealed99[str]

# block 100: generic arity stress
class Multi100(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi100[int]
class Single100(Generic[T1]):
    item: T1
Single100[int, str]
Alias100: TypeAlias = Single100[T2]
Alias100[int, str]
class Sealed100(Single100[int]):
    pass
Sealed100[str]

# block 101: generic arity stress
class Multi101(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi101[int]
class Single101(Generic[T1]):
    item: T1
Single101[int, str]
Alias101: TypeAlias = Single101[T2]
Alias101[int, str]
class Sealed101(Single101[int]):
    pass
Sealed101[str]

# block 102: generic arity stress
class Multi102(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi102[int]
class Single102(Generic[T1]):
    item: T1
Single102[int, str]
Alias102: TypeAlias = Single102[T2]
Alias102[int, str]
class Sealed102(Single102[int]):
    pass
Sealed102[str]

# block 103: generic arity stress
class Multi103(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi103[int]
class Single103(Generic[T1]):
    item: T1
Single103[int, str]
Alias103: TypeAlias = Single103[T2]
Alias103[int, str]
class Sealed103(Single103[int]):
    pass
Sealed103[str]

# block 104: generic arity stress
class Multi104(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi104[int]
class Single104(Generic[T1]):
    item: T1
Single104[int, str]
Alias104: TypeAlias = Single104[T2]
Alias104[int, str]
class Sealed104(Single104[int]):
    pass
Sealed104[str]

# block 105: generic arity stress
class Multi105(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi105[int]
class Single105(Generic[T1]):
    item: T1
Single105[int, str]
Alias105: TypeAlias = Single105[T2]
Alias105[int, str]
class Sealed105(Single105[int]):
    pass
Sealed105[str]

# block 106: generic arity stress
class Multi106(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi106[int]
class Single106(Generic[T1]):
    item: T1
Single106[int, str]
Alias106: TypeAlias = Single106[T2]
Alias106[int, str]
class Sealed106(Single106[int]):
    pass
Sealed106[str]

# block 107: generic arity stress
class Multi107(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi107[int]
class Single107(Generic[T1]):
    item: T1
Single107[int, str]
Alias107: TypeAlias = Single107[T2]
Alias107[int, str]
class Sealed107(Single107[int]):
    pass
Sealed107[str]

# block 108: generic arity stress
class Multi108(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi108[int]
class Single108(Generic[T1]):
    item: T1
Single108[int, str]
Alias108: TypeAlias = Single108[T2]
Alias108[int, str]
class Sealed108(Single108[int]):
    pass
Sealed108[str]

# block 109: generic arity stress
class Multi109(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi109[int]
class Single109(Generic[T1]):
    item: T1
Single109[int, str]
Alias109: TypeAlias = Single109[T2]
Alias109[int, str]
class Sealed109(Single109[int]):
    pass
Sealed109[str]

# block 110: generic arity stress
class Multi110(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi110[int]
class Single110(Generic[T1]):
    item: T1
Single110[int, str]
Alias110: TypeAlias = Single110[T2]
Alias110[int, str]
class Sealed110(Single110[int]):
    pass
Sealed110[str]

# block 111: generic arity stress
class Multi111(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi111[int]
class Single111(Generic[T1]):
    item: T1
Single111[int, str]
Alias111: TypeAlias = Single111[T2]
Alias111[int, str]
class Sealed111(Single111[int]):
    pass
Sealed111[str]

# block 112: generic arity stress
class Multi112(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi112[int]
class Single112(Generic[T1]):
    item: T1
Single112[int, str]
Alias112: TypeAlias = Single112[T2]
Alias112[int, str]
class Sealed112(Single112[int]):
    pass
Sealed112[str]

# block 113: generic arity stress
class Multi113(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi113[int]
class Single113(Generic[T1]):
    item: T1
Single113[int, str]
Alias113: TypeAlias = Single113[T2]
Alias113[int, str]
class Sealed113(Single113[int]):
    pass
Sealed113[str]

# block 114: generic arity stress
class Multi114(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi114[int]
class Single114(Generic[T1]):
    item: T1
Single114[int, str]
Alias114: TypeAlias = Single114[T2]
Alias114[int, str]
class Sealed114(Single114[int]):
    pass
Sealed114[str]

# block 115: generic arity stress
class Multi115(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi115[int]
class Single115(Generic[T1]):
    item: T1
Single115[int, str]
Alias115: TypeAlias = Single115[T2]
Alias115[int, str]
class Sealed115(Single115[int]):
    pass
Sealed115[str]

# block 116: generic arity stress
class Multi116(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi116[int]
class Single116(Generic[T1]):
    item: T1
Single116[int, str]
Alias116: TypeAlias = Single116[T2]
Alias116[int, str]
class Sealed116(Single116[int]):
    pass
Sealed116[str]

# block 117: generic arity stress
class Multi117(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi117[int]
class Single117(Generic[T1]):
    item: T1
Single117[int, str]
Alias117: TypeAlias = Single117[T2]
Alias117[int, str]
class Sealed117(Single117[int]):
    pass
Sealed117[str]

# block 118: generic arity stress
class Multi118(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi118[int]
class Single118(Generic[T1]):
    item: T1
Single118[int, str]
Alias118: TypeAlias = Single118[T2]
Alias118[int, str]
class Sealed118(Single118[int]):
    pass
Sealed118[str]

# block 119: generic arity stress
class Multi119(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi119[int]
class Single119(Generic[T1]):
    item: T1
Single119[int, str]
Alias119: TypeAlias = Single119[T2]
Alias119[int, str]
class Sealed119(Single119[int]):
    pass
Sealed119[str]

# block 120: generic arity stress
class Multi120(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi120[int]
class Single120(Generic[T1]):
    item: T1
Single120[int, str]
Alias120: TypeAlias = Single120[T2]
Alias120[int, str]
class Sealed120(Single120[int]):
    pass
Sealed120[str]

# block 121: generic arity stress
class Multi121(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi121[int]
class Single121(Generic[T1]):
    item: T1
Single121[int, str]
Alias121: TypeAlias = Single121[T2]
Alias121[int, str]
class Sealed121(Single121[int]):
    pass
Sealed121[str]

# block 122: generic arity stress
class Multi122(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi122[int]
class Single122(Generic[T1]):
    item: T1
Single122[int, str]
Alias122: TypeAlias = Single122[T2]
Alias122[int, str]
class Sealed122(Single122[int]):
    pass
Sealed122[str]

# block 123: generic arity stress
class Multi123(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi123[int]
class Single123(Generic[T1]):
    item: T1
Single123[int, str]
Alias123: TypeAlias = Single123[T2]
Alias123[int, str]
class Sealed123(Single123[int]):
    pass
Sealed123[str]

# block 124: generic arity stress
class Multi124(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi124[int]
class Single124(Generic[T1]):
    item: T1
Single124[int, str]
Alias124: TypeAlias = Single124[T2]
Alias124[int, str]
class Sealed124(Single124[int]):
    pass
Sealed124[str]

# block 125: generic arity stress
class Multi125(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi125[int]
class Single125(Generic[T1]):
    item: T1
Single125[int, str]
Alias125: TypeAlias = Single125[T2]
Alias125[int, str]
class Sealed125(Single125[int]):
    pass
Sealed125[str]

# block 126: generic arity stress
class Multi126(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi126[int]
class Single126(Generic[T1]):
    item: T1
Single126[int, str]
Alias126: TypeAlias = Single126[T2]
Alias126[int, str]
class Sealed126(Single126[int]):
    pass
Sealed126[str]

# block 127: generic arity stress
class Multi127(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi127[int]
class Single127(Generic[T1]):
    item: T1
Single127[int, str]
Alias127: TypeAlias = Single127[T2]
Alias127[int, str]
class Sealed127(Single127[int]):
    pass
Sealed127[str]

# block 128: generic arity stress
class Multi128(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi128[int]
class Single128(Generic[T1]):
    item: T1
Single128[int, str]
Alias128: TypeAlias = Single128[T2]
Alias128[int, str]
class Sealed128(Single128[int]):
    pass
Sealed128[str]

# block 129: generic arity stress
class Multi129(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi129[int]
class Single129(Generic[T1]):
    item: T1
Single129[int, str]
Alias129: TypeAlias = Single129[T2]
Alias129[int, str]
class Sealed129(Single129[int]):
    pass
Sealed129[str]

# block 130: generic arity stress
class Multi130(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi130[int]
class Single130(Generic[T1]):
    item: T1
Single130[int, str]
Alias130: TypeAlias = Single130[T2]
Alias130[int, str]
class Sealed130(Single130[int]):
    pass
Sealed130[str]

# block 131: generic arity stress
class Multi131(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi131[int]
class Single131(Generic[T1]):
    item: T1
Single131[int, str]
Alias131: TypeAlias = Single131[T2]
Alias131[int, str]
class Sealed131(Single131[int]):
    pass
Sealed131[str]

# block 132: generic arity stress
class Multi132(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi132[int]
class Single132(Generic[T1]):
    item: T1
Single132[int, str]
Alias132: TypeAlias = Single132[T2]
Alias132[int, str]
class Sealed132(Single132[int]):
    pass
Sealed132[str]

# block 133: generic arity stress
class Multi133(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi133[int]
class Single133(Generic[T1]):
    item: T1
Single133[int, str]
Alias133: TypeAlias = Single133[T2]
Alias133[int, str]
class Sealed133(Single133[int]):
    pass
Sealed133[str]

# block 134: generic arity stress
class Multi134(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi134[int]
class Single134(Generic[T1]):
    item: T1
Single134[int, str]
Alias134: TypeAlias = Single134[T2]
Alias134[int, str]
class Sealed134(Single134[int]):
    pass
Sealed134[str]

# block 135: generic arity stress
class Multi135(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi135[int]
class Single135(Generic[T1]):
    item: T1
Single135[int, str]
Alias135: TypeAlias = Single135[T2]
Alias135[int, str]
class Sealed135(Single135[int]):
    pass
Sealed135[str]

# block 136: generic arity stress
class Multi136(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi136[int]
class Single136(Generic[T1]):
    item: T1
Single136[int, str]
Alias136: TypeAlias = Single136[T2]
Alias136[int, str]
class Sealed136(Single136[int]):
    pass
Sealed136[str]

# block 137: generic arity stress
class Multi137(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi137[int]
class Single137(Generic[T1]):
    item: T1
Single137[int, str]
Alias137: TypeAlias = Single137[T2]
Alias137[int, str]
class Sealed137(Single137[int]):
    pass
Sealed137[str]

# block 138: generic arity stress
class Multi138(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi138[int]
class Single138(Generic[T1]):
    item: T1
Single138[int, str]
Alias138: TypeAlias = Single138[T2]
Alias138[int, str]
class Sealed138(Single138[int]):
    pass
Sealed138[str]

# block 139: generic arity stress
class Multi139(Generic[T1, T2, DefaultStrT]):
    first: T1
    tail: DefaultStrT
Multi139[int]
class Single139(Generic[T1]):
    item: T1
Single139[int, str]
Alias139: TypeAlias = Single139[T2]
Alias139[int, str]
class Sealed139(Single139[int]):
    pass
Sealed139[str]

