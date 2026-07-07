# Benchmark stress fixture: constructors_call_init
# Repeats 5 rotating constructor-call error patterns (6 error sites per block).
from dataclasses import dataclass
from typing import Generic, NamedTuple, TypeVar

T = TypeVar("T")
T1 = TypeVar("T1")
T2 = TypeVar("T2")

class GenB0(Generic[T]):
    def __init__(self: "GenB0[int]") -> None: ...

GenB0[str]()

class Plain0:
    pass

Plain0(1)

class GenD0(Generic[T1, T2]):
    def __init__(self: "GenD0[T2, T1]") -> None:
        pass

class Point0(NamedTuple):
    x: int
    y: int

Point0(1, 2, 3)
Point0(1, "no")

@dataclass
class Data0:
    a: int

Data0(a=1, bogus=2)

class GenB1(Generic[T]):
    def __init__(self: "GenB1[int]") -> None: ...

GenB1[str]()

class Plain1:
    pass

Plain1(1)

class GenD1(Generic[T1, T2]):
    def __init__(self: "GenD1[T2, T1]") -> None:
        pass

class Point1(NamedTuple):
    x: int
    y: int

Point1(1, 2, 3)
Point1(1, "no")

@dataclass
class Data1:
    a: int

Data1(a=1, bogus=2)

class GenB2(Generic[T]):
    def __init__(self: "GenB2[int]") -> None: ...

GenB2[str]()

class Plain2:
    pass

Plain2(1)

class GenD2(Generic[T1, T2]):
    def __init__(self: "GenD2[T2, T1]") -> None:
        pass

class Point2(NamedTuple):
    x: int
    y: int

Point2(1, 2, 3)
Point2(1, "no")

@dataclass
class Data2:
    a: int

Data2(a=1, bogus=2)

class GenB3(Generic[T]):
    def __init__(self: "GenB3[int]") -> None: ...

GenB3[str]()

class Plain3:
    pass

Plain3(1)

class GenD3(Generic[T1, T2]):
    def __init__(self: "GenD3[T2, T1]") -> None:
        pass

class Point3(NamedTuple):
    x: int
    y: int

Point3(1, 2, 3)
Point3(1, "no")

@dataclass
class Data3:
    a: int

Data3(a=1, bogus=2)

class GenB4(Generic[T]):
    def __init__(self: "GenB4[int]") -> None: ...

GenB4[str]()

class Plain4:
    pass

Plain4(1)

class GenD4(Generic[T1, T2]):
    def __init__(self: "GenD4[T2, T1]") -> None:
        pass

class Point4(NamedTuple):
    x: int
    y: int

Point4(1, 2, 3)
Point4(1, "no")

@dataclass
class Data4:
    a: int

Data4(a=1, bogus=2)

class GenB5(Generic[T]):
    def __init__(self: "GenB5[int]") -> None: ...

GenB5[str]()

class Plain5:
    pass

Plain5(1)

class GenD5(Generic[T1, T2]):
    def __init__(self: "GenD5[T2, T1]") -> None:
        pass

class Point5(NamedTuple):
    x: int
    y: int

Point5(1, 2, 3)
Point5(1, "no")

@dataclass
class Data5:
    a: int

Data5(a=1, bogus=2)

class GenB6(Generic[T]):
    def __init__(self: "GenB6[int]") -> None: ...

GenB6[str]()

class Plain6:
    pass

Plain6(1)

class GenD6(Generic[T1, T2]):
    def __init__(self: "GenD6[T2, T1]") -> None:
        pass

class Point6(NamedTuple):
    x: int
    y: int

Point6(1, 2, 3)
Point6(1, "no")

@dataclass
class Data6:
    a: int

Data6(a=1, bogus=2)

class GenB7(Generic[T]):
    def __init__(self: "GenB7[int]") -> None: ...

GenB7[str]()

class Plain7:
    pass

Plain7(1)

class GenD7(Generic[T1, T2]):
    def __init__(self: "GenD7[T2, T1]") -> None:
        pass

class Point7(NamedTuple):
    x: int
    y: int

Point7(1, 2, 3)
Point7(1, "no")

@dataclass
class Data7:
    a: int

Data7(a=1, bogus=2)

class GenB8(Generic[T]):
    def __init__(self: "GenB8[int]") -> None: ...

GenB8[str]()

class Plain8:
    pass

Plain8(1)

class GenD8(Generic[T1, T2]):
    def __init__(self: "GenD8[T2, T1]") -> None:
        pass

class Point8(NamedTuple):
    x: int
    y: int

Point8(1, 2, 3)
Point8(1, "no")

@dataclass
class Data8:
    a: int

Data8(a=1, bogus=2)

class GenB9(Generic[T]):
    def __init__(self: "GenB9[int]") -> None: ...

GenB9[str]()

class Plain9:
    pass

Plain9(1)

class GenD9(Generic[T1, T2]):
    def __init__(self: "GenD9[T2, T1]") -> None:
        pass

class Point9(NamedTuple):
    x: int
    y: int

Point9(1, 2, 3)
Point9(1, "no")

@dataclass
class Data9:
    a: int

Data9(a=1, bogus=2)

class GenB10(Generic[T]):
    def __init__(self: "GenB10[int]") -> None: ...

GenB10[str]()

class Plain10:
    pass

Plain10(1)

class GenD10(Generic[T1, T2]):
    def __init__(self: "GenD10[T2, T1]") -> None:
        pass

class Point10(NamedTuple):
    x: int
    y: int

Point10(1, 2, 3)
Point10(1, "no")

@dataclass
class Data10:
    a: int

Data10(a=1, bogus=2)

class GenB11(Generic[T]):
    def __init__(self: "GenB11[int]") -> None: ...

GenB11[str]()

class Plain11:
    pass

Plain11(1)

class GenD11(Generic[T1, T2]):
    def __init__(self: "GenD11[T2, T1]") -> None:
        pass

class Point11(NamedTuple):
    x: int
    y: int

Point11(1, 2, 3)
Point11(1, "no")

@dataclass
class Data11:
    a: int

Data11(a=1, bogus=2)

class GenB12(Generic[T]):
    def __init__(self: "GenB12[int]") -> None: ...

GenB12[str]()

class Plain12:
    pass

Plain12(1)

class GenD12(Generic[T1, T2]):
    def __init__(self: "GenD12[T2, T1]") -> None:
        pass

class Point12(NamedTuple):
    x: int
    y: int

Point12(1, 2, 3)
Point12(1, "no")

@dataclass
class Data12:
    a: int

Data12(a=1, bogus=2)

class GenB13(Generic[T]):
    def __init__(self: "GenB13[int]") -> None: ...

GenB13[str]()

class Plain13:
    pass

Plain13(1)

class GenD13(Generic[T1, T2]):
    def __init__(self: "GenD13[T2, T1]") -> None:
        pass

class Point13(NamedTuple):
    x: int
    y: int

Point13(1, 2, 3)
Point13(1, "no")

@dataclass
class Data13:
    a: int

Data13(a=1, bogus=2)

class GenB14(Generic[T]):
    def __init__(self: "GenB14[int]") -> None: ...

GenB14[str]()

class Plain14:
    pass

Plain14(1)

class GenD14(Generic[T1, T2]):
    def __init__(self: "GenD14[T2, T1]") -> None:
        pass

class Point14(NamedTuple):
    x: int
    y: int

Point14(1, 2, 3)
Point14(1, "no")

@dataclass
class Data14:
    a: int

Data14(a=1, bogus=2)

class GenB15(Generic[T]):
    def __init__(self: "GenB15[int]") -> None: ...

GenB15[str]()

class Plain15:
    pass

Plain15(1)

class GenD15(Generic[T1, T2]):
    def __init__(self: "GenD15[T2, T1]") -> None:
        pass

class Point15(NamedTuple):
    x: int
    y: int

Point15(1, 2, 3)
Point15(1, "no")

@dataclass
class Data15:
    a: int

Data15(a=1, bogus=2)

class GenB16(Generic[T]):
    def __init__(self: "GenB16[int]") -> None: ...

GenB16[str]()

class Plain16:
    pass

Plain16(1)

class GenD16(Generic[T1, T2]):
    def __init__(self: "GenD16[T2, T1]") -> None:
        pass

class Point16(NamedTuple):
    x: int
    y: int

Point16(1, 2, 3)
Point16(1, "no")

@dataclass
class Data16:
    a: int

Data16(a=1, bogus=2)

class GenB17(Generic[T]):
    def __init__(self: "GenB17[int]") -> None: ...

GenB17[str]()

class Plain17:
    pass

Plain17(1)

class GenD17(Generic[T1, T2]):
    def __init__(self: "GenD17[T2, T1]") -> None:
        pass

class Point17(NamedTuple):
    x: int
    y: int

Point17(1, 2, 3)
Point17(1, "no")

@dataclass
class Data17:
    a: int

Data17(a=1, bogus=2)

class GenB18(Generic[T]):
    def __init__(self: "GenB18[int]") -> None: ...

GenB18[str]()

class Plain18:
    pass

Plain18(1)

class GenD18(Generic[T1, T2]):
    def __init__(self: "GenD18[T2, T1]") -> None:
        pass

class Point18(NamedTuple):
    x: int
    y: int

Point18(1, 2, 3)
Point18(1, "no")

@dataclass
class Data18:
    a: int

Data18(a=1, bogus=2)

class GenB19(Generic[T]):
    def __init__(self: "GenB19[int]") -> None: ...

GenB19[str]()

class Plain19:
    pass

Plain19(1)

class GenD19(Generic[T1, T2]):
    def __init__(self: "GenD19[T2, T1]") -> None:
        pass

class Point19(NamedTuple):
    x: int
    y: int

Point19(1, 2, 3)
Point19(1, "no")

@dataclass
class Data19:
    a: int

Data19(a=1, bogus=2)

class GenB20(Generic[T]):
    def __init__(self: "GenB20[int]") -> None: ...

GenB20[str]()

class Plain20:
    pass

Plain20(1)

class GenD20(Generic[T1, T2]):
    def __init__(self: "GenD20[T2, T1]") -> None:
        pass

class Point20(NamedTuple):
    x: int
    y: int

Point20(1, 2, 3)
Point20(1, "no")

@dataclass
class Data20:
    a: int

Data20(a=1, bogus=2)

class GenB21(Generic[T]):
    def __init__(self: "GenB21[int]") -> None: ...

GenB21[str]()

class Plain21:
    pass

Plain21(1)

class GenD21(Generic[T1, T2]):
    def __init__(self: "GenD21[T2, T1]") -> None:
        pass

class Point21(NamedTuple):
    x: int
    y: int

Point21(1, 2, 3)
Point21(1, "no")

@dataclass
class Data21:
    a: int

Data21(a=1, bogus=2)

class GenB22(Generic[T]):
    def __init__(self: "GenB22[int]") -> None: ...

GenB22[str]()

class Plain22:
    pass

Plain22(1)

class GenD22(Generic[T1, T2]):
    def __init__(self: "GenD22[T2, T1]") -> None:
        pass

class Point22(NamedTuple):
    x: int
    y: int

Point22(1, 2, 3)
Point22(1, "no")

@dataclass
class Data22:
    a: int

Data22(a=1, bogus=2)

class GenB23(Generic[T]):
    def __init__(self: "GenB23[int]") -> None: ...

GenB23[str]()

class Plain23:
    pass

Plain23(1)

class GenD23(Generic[T1, T2]):
    def __init__(self: "GenD23[T2, T1]") -> None:
        pass

class Point23(NamedTuple):
    x: int
    y: int

Point23(1, 2, 3)
Point23(1, "no")

@dataclass
class Data23:
    a: int

Data23(a=1, bogus=2)

class GenB24(Generic[T]):
    def __init__(self: "GenB24[int]") -> None: ...

GenB24[str]()

class Plain24:
    pass

Plain24(1)

class GenD24(Generic[T1, T2]):
    def __init__(self: "GenD24[T2, T1]") -> None:
        pass

class Point24(NamedTuple):
    x: int
    y: int

Point24(1, 2, 3)
Point24(1, "no")

@dataclass
class Data24:
    a: int

Data24(a=1, bogus=2)

class GenB25(Generic[T]):
    def __init__(self: "GenB25[int]") -> None: ...

GenB25[str]()

class Plain25:
    pass

Plain25(1)

class GenD25(Generic[T1, T2]):
    def __init__(self: "GenD25[T2, T1]") -> None:
        pass

class Point25(NamedTuple):
    x: int
    y: int

Point25(1, 2, 3)
Point25(1, "no")

@dataclass
class Data25:
    a: int

Data25(a=1, bogus=2)

class GenB26(Generic[T]):
    def __init__(self: "GenB26[int]") -> None: ...

GenB26[str]()

class Plain26:
    pass

Plain26(1)

class GenD26(Generic[T1, T2]):
    def __init__(self: "GenD26[T2, T1]") -> None:
        pass

class Point26(NamedTuple):
    x: int
    y: int

Point26(1, 2, 3)
Point26(1, "no")

@dataclass
class Data26:
    a: int

Data26(a=1, bogus=2)

class GenB27(Generic[T]):
    def __init__(self: "GenB27[int]") -> None: ...

GenB27[str]()

class Plain27:
    pass

Plain27(1)

class GenD27(Generic[T1, T2]):
    def __init__(self: "GenD27[T2, T1]") -> None:
        pass

class Point27(NamedTuple):
    x: int
    y: int

Point27(1, 2, 3)
Point27(1, "no")

@dataclass
class Data27:
    a: int

Data27(a=1, bogus=2)

class GenB28(Generic[T]):
    def __init__(self: "GenB28[int]") -> None: ...

GenB28[str]()

class Plain28:
    pass

Plain28(1)

class GenD28(Generic[T1, T2]):
    def __init__(self: "GenD28[T2, T1]") -> None:
        pass

class Point28(NamedTuple):
    x: int
    y: int

Point28(1, 2, 3)
Point28(1, "no")

@dataclass
class Data28:
    a: int

Data28(a=1, bogus=2)

class GenB29(Generic[T]):
    def __init__(self: "GenB29[int]") -> None: ...

GenB29[str]()

class Plain29:
    pass

Plain29(1)

class GenD29(Generic[T1, T2]):
    def __init__(self: "GenD29[T2, T1]") -> None:
        pass

class Point29(NamedTuple):
    x: int
    y: int

Point29(1, 2, 3)
Point29(1, "no")

@dataclass
class Data29:
    a: int

Data29(a=1, bogus=2)

class GenB30(Generic[T]):
    def __init__(self: "GenB30[int]") -> None: ...

GenB30[str]()

class Plain30:
    pass

Plain30(1)

class GenD30(Generic[T1, T2]):
    def __init__(self: "GenD30[T2, T1]") -> None:
        pass

class Point30(NamedTuple):
    x: int
    y: int

Point30(1, 2, 3)
Point30(1, "no")

@dataclass
class Data30:
    a: int

Data30(a=1, bogus=2)

class GenB31(Generic[T]):
    def __init__(self: "GenB31[int]") -> None: ...

GenB31[str]()

class Plain31:
    pass

Plain31(1)

class GenD31(Generic[T1, T2]):
    def __init__(self: "GenD31[T2, T1]") -> None:
        pass

class Point31(NamedTuple):
    x: int
    y: int

Point31(1, 2, 3)
Point31(1, "no")

@dataclass
class Data31:
    a: int

Data31(a=1, bogus=2)

class GenB32(Generic[T]):
    def __init__(self: "GenB32[int]") -> None: ...

GenB32[str]()

class Plain32:
    pass

Plain32(1)

class GenD32(Generic[T1, T2]):
    def __init__(self: "GenD32[T2, T1]") -> None:
        pass

class Point32(NamedTuple):
    x: int
    y: int

Point32(1, 2, 3)
Point32(1, "no")

@dataclass
class Data32:
    a: int

Data32(a=1, bogus=2)

class GenB33(Generic[T]):
    def __init__(self: "GenB33[int]") -> None: ...

GenB33[str]()

class Plain33:
    pass

Plain33(1)

class GenD33(Generic[T1, T2]):
    def __init__(self: "GenD33[T2, T1]") -> None:
        pass

class Point33(NamedTuple):
    x: int
    y: int

Point33(1, 2, 3)
Point33(1, "no")

@dataclass
class Data33:
    a: int

Data33(a=1, bogus=2)

class GenB34(Generic[T]):
    def __init__(self: "GenB34[int]") -> None: ...

GenB34[str]()

class Plain34:
    pass

Plain34(1)

class GenD34(Generic[T1, T2]):
    def __init__(self: "GenD34[T2, T1]") -> None:
        pass

class Point34(NamedTuple):
    x: int
    y: int

Point34(1, 2, 3)
Point34(1, "no")

@dataclass
class Data34:
    a: int

Data34(a=1, bogus=2)

class GenB35(Generic[T]):
    def __init__(self: "GenB35[int]") -> None: ...

GenB35[str]()

class Plain35:
    pass

Plain35(1)

class GenD35(Generic[T1, T2]):
    def __init__(self: "GenD35[T2, T1]") -> None:
        pass

class Point35(NamedTuple):
    x: int
    y: int

Point35(1, 2, 3)
Point35(1, "no")

@dataclass
class Data35:
    a: int

Data35(a=1, bogus=2)

class GenB36(Generic[T]):
    def __init__(self: "GenB36[int]") -> None: ...

GenB36[str]()

class Plain36:
    pass

Plain36(1)

class GenD36(Generic[T1, T2]):
    def __init__(self: "GenD36[T2, T1]") -> None:
        pass

class Point36(NamedTuple):
    x: int
    y: int

Point36(1, 2, 3)
Point36(1, "no")

@dataclass
class Data36:
    a: int

Data36(a=1, bogus=2)

class GenB37(Generic[T]):
    def __init__(self: "GenB37[int]") -> None: ...

GenB37[str]()

class Plain37:
    pass

Plain37(1)

class GenD37(Generic[T1, T2]):
    def __init__(self: "GenD37[T2, T1]") -> None:
        pass

class Point37(NamedTuple):
    x: int
    y: int

Point37(1, 2, 3)
Point37(1, "no")

@dataclass
class Data37:
    a: int

Data37(a=1, bogus=2)

class GenB38(Generic[T]):
    def __init__(self: "GenB38[int]") -> None: ...

GenB38[str]()

class Plain38:
    pass

Plain38(1)

class GenD38(Generic[T1, T2]):
    def __init__(self: "GenD38[T2, T1]") -> None:
        pass

class Point38(NamedTuple):
    x: int
    y: int

Point38(1, 2, 3)
Point38(1, "no")

@dataclass
class Data38:
    a: int

Data38(a=1, bogus=2)

class GenB39(Generic[T]):
    def __init__(self: "GenB39[int]") -> None: ...

GenB39[str]()

class Plain39:
    pass

Plain39(1)

class GenD39(Generic[T1, T2]):
    def __init__(self: "GenD39[T2, T1]") -> None:
        pass

class Point39(NamedTuple):
    x: int
    y: int

Point39(1, 2, 3)
Point39(1, "no")

@dataclass
class Data39:
    a: int

Data39(a=1, bogus=2)

class GenB40(Generic[T]):
    def __init__(self: "GenB40[int]") -> None: ...

GenB40[str]()

class Plain40:
    pass

Plain40(1)

class GenD40(Generic[T1, T2]):
    def __init__(self: "GenD40[T2, T1]") -> None:
        pass

class Point40(NamedTuple):
    x: int
    y: int

Point40(1, 2, 3)
Point40(1, "no")

@dataclass
class Data40:
    a: int

Data40(a=1, bogus=2)

class GenB41(Generic[T]):
    def __init__(self: "GenB41[int]") -> None: ...

GenB41[str]()

class Plain41:
    pass

Plain41(1)

class GenD41(Generic[T1, T2]):
    def __init__(self: "GenD41[T2, T1]") -> None:
        pass

class Point41(NamedTuple):
    x: int
    y: int

Point41(1, 2, 3)
Point41(1, "no")

@dataclass
class Data41:
    a: int

Data41(a=1, bogus=2)

class GenB42(Generic[T]):
    def __init__(self: "GenB42[int]") -> None: ...

GenB42[str]()

class Plain42:
    pass

Plain42(1)

class GenD42(Generic[T1, T2]):
    def __init__(self: "GenD42[T2, T1]") -> None:
        pass

class Point42(NamedTuple):
    x: int
    y: int

Point42(1, 2, 3)
Point42(1, "no")

@dataclass
class Data42:
    a: int

Data42(a=1, bogus=2)

class GenB43(Generic[T]):
    def __init__(self: "GenB43[int]") -> None: ...

GenB43[str]()

class Plain43:
    pass

Plain43(1)

class GenD43(Generic[T1, T2]):
    def __init__(self: "GenD43[T2, T1]") -> None:
        pass

class Point43(NamedTuple):
    x: int
    y: int

Point43(1, 2, 3)
Point43(1, "no")

@dataclass
class Data43:
    a: int

Data43(a=1, bogus=2)

class GenB44(Generic[T]):
    def __init__(self: "GenB44[int]") -> None: ...

GenB44[str]()

class Plain44:
    pass

Plain44(1)

class GenD44(Generic[T1, T2]):
    def __init__(self: "GenD44[T2, T1]") -> None:
        pass

class Point44(NamedTuple):
    x: int
    y: int

Point44(1, 2, 3)
Point44(1, "no")

@dataclass
class Data44:
    a: int

Data44(a=1, bogus=2)

class GenB45(Generic[T]):
    def __init__(self: "GenB45[int]") -> None: ...

GenB45[str]()

class Plain45:
    pass

Plain45(1)

class GenD45(Generic[T1, T2]):
    def __init__(self: "GenD45[T2, T1]") -> None:
        pass

class Point45(NamedTuple):
    x: int
    y: int

Point45(1, 2, 3)
Point45(1, "no")

@dataclass
class Data45:
    a: int

Data45(a=1, bogus=2)

class GenB46(Generic[T]):
    def __init__(self: "GenB46[int]") -> None: ...

GenB46[str]()

class Plain46:
    pass

Plain46(1)

class GenD46(Generic[T1, T2]):
    def __init__(self: "GenD46[T2, T1]") -> None:
        pass

class Point46(NamedTuple):
    x: int
    y: int

Point46(1, 2, 3)
Point46(1, "no")

@dataclass
class Data46:
    a: int

Data46(a=1, bogus=2)

class GenB47(Generic[T]):
    def __init__(self: "GenB47[int]") -> None: ...

GenB47[str]()

class Plain47:
    pass

Plain47(1)

class GenD47(Generic[T1, T2]):
    def __init__(self: "GenD47[T2, T1]") -> None:
        pass

class Point47(NamedTuple):
    x: int
    y: int

Point47(1, 2, 3)
Point47(1, "no")

@dataclass
class Data47:
    a: int

Data47(a=1, bogus=2)

class GenB48(Generic[T]):
    def __init__(self: "GenB48[int]") -> None: ...

GenB48[str]()

class Plain48:
    pass

Plain48(1)

class GenD48(Generic[T1, T2]):
    def __init__(self: "GenD48[T2, T1]") -> None:
        pass

class Point48(NamedTuple):
    x: int
    y: int

Point48(1, 2, 3)
Point48(1, "no")

@dataclass
class Data48:
    a: int

Data48(a=1, bogus=2)

class GenB49(Generic[T]):
    def __init__(self: "GenB49[int]") -> None: ...

GenB49[str]()

class Plain49:
    pass

Plain49(1)

class GenD49(Generic[T1, T2]):
    def __init__(self: "GenD49[T2, T1]") -> None:
        pass

class Point49(NamedTuple):
    x: int
    y: int

Point49(1, 2, 3)
Point49(1, "no")

@dataclass
class Data49:
    a: int

Data49(a=1, bogus=2)

class GenB50(Generic[T]):
    def __init__(self: "GenB50[int]") -> None: ...

GenB50[str]()

class Plain50:
    pass

Plain50(1)

class GenD50(Generic[T1, T2]):
    def __init__(self: "GenD50[T2, T1]") -> None:
        pass

class Point50(NamedTuple):
    x: int
    y: int

Point50(1, 2, 3)
Point50(1, "no")

@dataclass
class Data50:
    a: int

Data50(a=1, bogus=2)

class GenB51(Generic[T]):
    def __init__(self: "GenB51[int]") -> None: ...

GenB51[str]()

class Plain51:
    pass

Plain51(1)

class GenD51(Generic[T1, T2]):
    def __init__(self: "GenD51[T2, T1]") -> None:
        pass

class Point51(NamedTuple):
    x: int
    y: int

Point51(1, 2, 3)
Point51(1, "no")

@dataclass
class Data51:
    a: int

Data51(a=1, bogus=2)

class GenB52(Generic[T]):
    def __init__(self: "GenB52[int]") -> None: ...

GenB52[str]()

class Plain52:
    pass

Plain52(1)

class GenD52(Generic[T1, T2]):
    def __init__(self: "GenD52[T2, T1]") -> None:
        pass

class Point52(NamedTuple):
    x: int
    y: int

Point52(1, 2, 3)
Point52(1, "no")

@dataclass
class Data52:
    a: int

Data52(a=1, bogus=2)

class GenB53(Generic[T]):
    def __init__(self: "GenB53[int]") -> None: ...

GenB53[str]()

class Plain53:
    pass

Plain53(1)

class GenD53(Generic[T1, T2]):
    def __init__(self: "GenD53[T2, T1]") -> None:
        pass

class Point53(NamedTuple):
    x: int
    y: int

Point53(1, 2, 3)
Point53(1, "no")

@dataclass
class Data53:
    a: int

Data53(a=1, bogus=2)

class GenB54(Generic[T]):
    def __init__(self: "GenB54[int]") -> None: ...

GenB54[str]()

class Plain54:
    pass

Plain54(1)

class GenD54(Generic[T1, T2]):
    def __init__(self: "GenD54[T2, T1]") -> None:
        pass

class Point54(NamedTuple):
    x: int
    y: int

Point54(1, 2, 3)
Point54(1, "no")

@dataclass
class Data54:
    a: int

Data54(a=1, bogus=2)

class GenB55(Generic[T]):
    def __init__(self: "GenB55[int]") -> None: ...

GenB55[str]()

class Plain55:
    pass

Plain55(1)

class GenD55(Generic[T1, T2]):
    def __init__(self: "GenD55[T2, T1]") -> None:
        pass

class Point55(NamedTuple):
    x: int
    y: int

Point55(1, 2, 3)
Point55(1, "no")

@dataclass
class Data55:
    a: int

Data55(a=1, bogus=2)

class GenB56(Generic[T]):
    def __init__(self: "GenB56[int]") -> None: ...

GenB56[str]()

class Plain56:
    pass

Plain56(1)

class GenD56(Generic[T1, T2]):
    def __init__(self: "GenD56[T2, T1]") -> None:
        pass

class Point56(NamedTuple):
    x: int
    y: int

Point56(1, 2, 3)
Point56(1, "no")

@dataclass
class Data56:
    a: int

Data56(a=1, bogus=2)

class GenB57(Generic[T]):
    def __init__(self: "GenB57[int]") -> None: ...

GenB57[str]()

class Plain57:
    pass

Plain57(1)

class GenD57(Generic[T1, T2]):
    def __init__(self: "GenD57[T2, T1]") -> None:
        pass

class Point57(NamedTuple):
    x: int
    y: int

Point57(1, 2, 3)
Point57(1, "no")

@dataclass
class Data57:
    a: int

Data57(a=1, bogus=2)

class GenB58(Generic[T]):
    def __init__(self: "GenB58[int]") -> None: ...

GenB58[str]()

class Plain58:
    pass

Plain58(1)

class GenD58(Generic[T1, T2]):
    def __init__(self: "GenD58[T2, T1]") -> None:
        pass

class Point58(NamedTuple):
    x: int
    y: int

Point58(1, 2, 3)
Point58(1, "no")

@dataclass
class Data58:
    a: int

Data58(a=1, bogus=2)

class GenB59(Generic[T]):
    def __init__(self: "GenB59[int]") -> None: ...

GenB59[str]()

class Plain59:
    pass

Plain59(1)

class GenD59(Generic[T1, T2]):
    def __init__(self: "GenD59[T2, T1]") -> None:
        pass

class Point59(NamedTuple):
    x: int
    y: int

Point59(1, 2, 3)
Point59(1, "no")

@dataclass
class Data59:
    a: int

Data59(a=1, bogus=2)

class GenB60(Generic[T]):
    def __init__(self: "GenB60[int]") -> None: ...

GenB60[str]()

class Plain60:
    pass

Plain60(1)

class GenD60(Generic[T1, T2]):
    def __init__(self: "GenD60[T2, T1]") -> None:
        pass

class Point60(NamedTuple):
    x: int
    y: int

Point60(1, 2, 3)
Point60(1, "no")

@dataclass
class Data60:
    a: int

Data60(a=1, bogus=2)

class GenB61(Generic[T]):
    def __init__(self: "GenB61[int]") -> None: ...

GenB61[str]()

class Plain61:
    pass

Plain61(1)

class GenD61(Generic[T1, T2]):
    def __init__(self: "GenD61[T2, T1]") -> None:
        pass

class Point61(NamedTuple):
    x: int
    y: int

Point61(1, 2, 3)
Point61(1, "no")

@dataclass
class Data61:
    a: int

Data61(a=1, bogus=2)

class GenB62(Generic[T]):
    def __init__(self: "GenB62[int]") -> None: ...

GenB62[str]()

class Plain62:
    pass

Plain62(1)

class GenD62(Generic[T1, T2]):
    def __init__(self: "GenD62[T2, T1]") -> None:
        pass

class Point62(NamedTuple):
    x: int
    y: int

Point62(1, 2, 3)
Point62(1, "no")

@dataclass
class Data62:
    a: int

Data62(a=1, bogus=2)

class GenB63(Generic[T]):
    def __init__(self: "GenB63[int]") -> None: ...

GenB63[str]()

class Plain63:
    pass

Plain63(1)

class GenD63(Generic[T1, T2]):
    def __init__(self: "GenD63[T2, T1]") -> None:
        pass

class Point63(NamedTuple):
    x: int
    y: int

Point63(1, 2, 3)
Point63(1, "no")

@dataclass
class Data63:
    a: int

Data63(a=1, bogus=2)

class GenB64(Generic[T]):
    def __init__(self: "GenB64[int]") -> None: ...

GenB64[str]()

class Plain64:
    pass

Plain64(1)

class GenD64(Generic[T1, T2]):
    def __init__(self: "GenD64[T2, T1]") -> None:
        pass

class Point64(NamedTuple):
    x: int
    y: int

Point64(1, 2, 3)
Point64(1, "no")

@dataclass
class Data64:
    a: int

Data64(a=1, bogus=2)

class GenB65(Generic[T]):
    def __init__(self: "GenB65[int]") -> None: ...

GenB65[str]()

class Plain65:
    pass

Plain65(1)

class GenD65(Generic[T1, T2]):
    def __init__(self: "GenD65[T2, T1]") -> None:
        pass

class Point65(NamedTuple):
    x: int
    y: int

Point65(1, 2, 3)
Point65(1, "no")

@dataclass
class Data65:
    a: int

Data65(a=1, bogus=2)

class GenB66(Generic[T]):
    def __init__(self: "GenB66[int]") -> None: ...

GenB66[str]()

class Plain66:
    pass

Plain66(1)

class GenD66(Generic[T1, T2]):
    def __init__(self: "GenD66[T2, T1]") -> None:
        pass

class Point66(NamedTuple):
    x: int
    y: int

Point66(1, 2, 3)
Point66(1, "no")

@dataclass
class Data66:
    a: int

Data66(a=1, bogus=2)

class GenB67(Generic[T]):
    def __init__(self: "GenB67[int]") -> None: ...

GenB67[str]()

class Plain67:
    pass

Plain67(1)

class GenD67(Generic[T1, T2]):
    def __init__(self: "GenD67[T2, T1]") -> None:
        pass

class Point67(NamedTuple):
    x: int
    y: int

Point67(1, 2, 3)
Point67(1, "no")

@dataclass
class Data67:
    a: int

Data67(a=1, bogus=2)

class GenB68(Generic[T]):
    def __init__(self: "GenB68[int]") -> None: ...

GenB68[str]()

class Plain68:
    pass

Plain68(1)

class GenD68(Generic[T1, T2]):
    def __init__(self: "GenD68[T2, T1]") -> None:
        pass

class Point68(NamedTuple):
    x: int
    y: int

Point68(1, 2, 3)
Point68(1, "no")

@dataclass
class Data68:
    a: int

Data68(a=1, bogus=2)

class GenB69(Generic[T]):
    def __init__(self: "GenB69[int]") -> None: ...

GenB69[str]()

class Plain69:
    pass

Plain69(1)

class GenD69(Generic[T1, T2]):
    def __init__(self: "GenD69[T2, T1]") -> None:
        pass

class Point69(NamedTuple):
    x: int
    y: int

Point69(1, 2, 3)
Point69(1, "no")

@dataclass
class Data69:
    a: int

Data69(a=1, bogus=2)

class GenB70(Generic[T]):
    def __init__(self: "GenB70[int]") -> None: ...

GenB70[str]()

class Plain70:
    pass

Plain70(1)

class GenD70(Generic[T1, T2]):
    def __init__(self: "GenD70[T2, T1]") -> None:
        pass

class Point70(NamedTuple):
    x: int
    y: int

Point70(1, 2, 3)
Point70(1, "no")

@dataclass
class Data70:
    a: int

Data70(a=1, bogus=2)

class GenB71(Generic[T]):
    def __init__(self: "GenB71[int]") -> None: ...

GenB71[str]()

class Plain71:
    pass

Plain71(1)

class GenD71(Generic[T1, T2]):
    def __init__(self: "GenD71[T2, T1]") -> None:
        pass

class Point71(NamedTuple):
    x: int
    y: int

Point71(1, 2, 3)
Point71(1, "no")

@dataclass
class Data71:
    a: int

Data71(a=1, bogus=2)

class GenB72(Generic[T]):
    def __init__(self: "GenB72[int]") -> None: ...

GenB72[str]()

class Plain72:
    pass

Plain72(1)

class GenD72(Generic[T1, T2]):
    def __init__(self: "GenD72[T2, T1]") -> None:
        pass

class Point72(NamedTuple):
    x: int
    y: int

Point72(1, 2, 3)
Point72(1, "no")

@dataclass
class Data72:
    a: int

Data72(a=1, bogus=2)

class GenB73(Generic[T]):
    def __init__(self: "GenB73[int]") -> None: ...

GenB73[str]()

class Plain73:
    pass

Plain73(1)

class GenD73(Generic[T1, T2]):
    def __init__(self: "GenD73[T2, T1]") -> None:
        pass

class Point73(NamedTuple):
    x: int
    y: int

Point73(1, 2, 3)
Point73(1, "no")

@dataclass
class Data73:
    a: int

Data73(a=1, bogus=2)

