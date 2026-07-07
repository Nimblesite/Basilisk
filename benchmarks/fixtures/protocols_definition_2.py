# Benchmark stress fixture for rule `protocols_definition_2`.
# Repeats protocol conformance violations in module-level annotated assignments:
# missing methods, missing instance variables, ClassVar member-kind mismatches,
# and assignment to non-protocol classes that inherit from a protocol.
from typing import ClassVar, Protocol

class Proto0(Protocol):
    val0: int

    def method0(self) -> None: ...

class Missing0:
    pass

class HasVarOnly0:
    val0: int = 0

class HasClassVar0:
    val0: ClassVar[int] = 0

    def method0(self) -> None:
        return None

class NonProto0(Proto0):
    val0: int = 0

    def method0(self) -> None:
        return None

bad_a0: Proto0 = Missing0()
bad_b0: Proto0 = HasVarOnly0()
bad_c0: Proto0 = HasClassVar0()
bad_d0: NonProto0 = HasClassVar0()
bad_e0: NonProto0 = Missing0()
bad_f0: Proto0 = Missing0()

class Proto1(Protocol):
    val1: int

    def method1(self) -> None: ...

class Missing1:
    pass

class HasVarOnly1:
    val1: int = 0

class HasClassVar1:
    val1: ClassVar[int] = 0

    def method1(self) -> None:
        return None

class NonProto1(Proto1):
    val1: int = 0

    def method1(self) -> None:
        return None

bad_a1: Proto1 = Missing1()
bad_b1: Proto1 = HasVarOnly1()
bad_c1: Proto1 = HasClassVar1()
bad_d1: NonProto1 = HasClassVar1()
bad_e1: NonProto1 = Missing1()
bad_f1: Proto1 = Missing1()

class Proto2(Protocol):
    val2: int

    def method2(self) -> None: ...

class Missing2:
    pass

class HasVarOnly2:
    val2: int = 0

class HasClassVar2:
    val2: ClassVar[int] = 0

    def method2(self) -> None:
        return None

class NonProto2(Proto2):
    val2: int = 0

    def method2(self) -> None:
        return None

bad_a2: Proto2 = Missing2()
bad_b2: Proto2 = HasVarOnly2()
bad_c2: Proto2 = HasClassVar2()
bad_d2: NonProto2 = HasClassVar2()
bad_e2: NonProto2 = Missing2()
bad_f2: Proto2 = Missing2()

class Proto3(Protocol):
    val3: int

    def method3(self) -> None: ...

class Missing3:
    pass

class HasVarOnly3:
    val3: int = 0

class HasClassVar3:
    val3: ClassVar[int] = 0

    def method3(self) -> None:
        return None

class NonProto3(Proto3):
    val3: int = 0

    def method3(self) -> None:
        return None

bad_a3: Proto3 = Missing3()
bad_b3: Proto3 = HasVarOnly3()
bad_c3: Proto3 = HasClassVar3()
bad_d3: NonProto3 = HasClassVar3()
bad_e3: NonProto3 = Missing3()
bad_f3: Proto3 = Missing3()

class Proto4(Protocol):
    val4: int

    def method4(self) -> None: ...

class Missing4:
    pass

class HasVarOnly4:
    val4: int = 0

class HasClassVar4:
    val4: ClassVar[int] = 0

    def method4(self) -> None:
        return None

class NonProto4(Proto4):
    val4: int = 0

    def method4(self) -> None:
        return None

bad_a4: Proto4 = Missing4()
bad_b4: Proto4 = HasVarOnly4()
bad_c4: Proto4 = HasClassVar4()
bad_d4: NonProto4 = HasClassVar4()
bad_e4: NonProto4 = Missing4()
bad_f4: Proto4 = Missing4()

class Proto5(Protocol):
    val5: int

    def method5(self) -> None: ...

class Missing5:
    pass

class HasVarOnly5:
    val5: int = 0

class HasClassVar5:
    val5: ClassVar[int] = 0

    def method5(self) -> None:
        return None

class NonProto5(Proto5):
    val5: int = 0

    def method5(self) -> None:
        return None

bad_a5: Proto5 = Missing5()
bad_b5: Proto5 = HasVarOnly5()
bad_c5: Proto5 = HasClassVar5()
bad_d5: NonProto5 = HasClassVar5()
bad_e5: NonProto5 = Missing5()
bad_f5: Proto5 = Missing5()

class Proto6(Protocol):
    val6: int

    def method6(self) -> None: ...

class Missing6:
    pass

class HasVarOnly6:
    val6: int = 0

class HasClassVar6:
    val6: ClassVar[int] = 0

    def method6(self) -> None:
        return None

class NonProto6(Proto6):
    val6: int = 0

    def method6(self) -> None:
        return None

bad_a6: Proto6 = Missing6()
bad_b6: Proto6 = HasVarOnly6()
bad_c6: Proto6 = HasClassVar6()
bad_d6: NonProto6 = HasClassVar6()
bad_e6: NonProto6 = Missing6()
bad_f6: Proto6 = Missing6()

class Proto7(Protocol):
    val7: int

    def method7(self) -> None: ...

class Missing7:
    pass

class HasVarOnly7:
    val7: int = 0

class HasClassVar7:
    val7: ClassVar[int] = 0

    def method7(self) -> None:
        return None

class NonProto7(Proto7):
    val7: int = 0

    def method7(self) -> None:
        return None

bad_a7: Proto7 = Missing7()
bad_b7: Proto7 = HasVarOnly7()
bad_c7: Proto7 = HasClassVar7()
bad_d7: NonProto7 = HasClassVar7()
bad_e7: NonProto7 = Missing7()
bad_f7: Proto7 = Missing7()

class Proto8(Protocol):
    val8: int

    def method8(self) -> None: ...

class Missing8:
    pass

class HasVarOnly8:
    val8: int = 0

class HasClassVar8:
    val8: ClassVar[int] = 0

    def method8(self) -> None:
        return None

class NonProto8(Proto8):
    val8: int = 0

    def method8(self) -> None:
        return None

bad_a8: Proto8 = Missing8()
bad_b8: Proto8 = HasVarOnly8()
bad_c8: Proto8 = HasClassVar8()
bad_d8: NonProto8 = HasClassVar8()
bad_e8: NonProto8 = Missing8()
bad_f8: Proto8 = Missing8()

class Proto9(Protocol):
    val9: int

    def method9(self) -> None: ...

class Missing9:
    pass

class HasVarOnly9:
    val9: int = 0

class HasClassVar9:
    val9: ClassVar[int] = 0

    def method9(self) -> None:
        return None

class NonProto9(Proto9):
    val9: int = 0

    def method9(self) -> None:
        return None

bad_a9: Proto9 = Missing9()
bad_b9: Proto9 = HasVarOnly9()
bad_c9: Proto9 = HasClassVar9()
bad_d9: NonProto9 = HasClassVar9()
bad_e9: NonProto9 = Missing9()
bad_f9: Proto9 = Missing9()

class Proto10(Protocol):
    val10: int

    def method10(self) -> None: ...

class Missing10:
    pass

class HasVarOnly10:
    val10: int = 0

class HasClassVar10:
    val10: ClassVar[int] = 0

    def method10(self) -> None:
        return None

class NonProto10(Proto10):
    val10: int = 0

    def method10(self) -> None:
        return None

bad_a10: Proto10 = Missing10()
bad_b10: Proto10 = HasVarOnly10()
bad_c10: Proto10 = HasClassVar10()
bad_d10: NonProto10 = HasClassVar10()
bad_e10: NonProto10 = Missing10()
bad_f10: Proto10 = Missing10()

class Proto11(Protocol):
    val11: int

    def method11(self) -> None: ...

class Missing11:
    pass

class HasVarOnly11:
    val11: int = 0

class HasClassVar11:
    val11: ClassVar[int] = 0

    def method11(self) -> None:
        return None

class NonProto11(Proto11):
    val11: int = 0

    def method11(self) -> None:
        return None

bad_a11: Proto11 = Missing11()
bad_b11: Proto11 = HasVarOnly11()
bad_c11: Proto11 = HasClassVar11()
bad_d11: NonProto11 = HasClassVar11()
bad_e11: NonProto11 = Missing11()
bad_f11: Proto11 = Missing11()

class Proto12(Protocol):
    val12: int

    def method12(self) -> None: ...

class Missing12:
    pass

class HasVarOnly12:
    val12: int = 0

class HasClassVar12:
    val12: ClassVar[int] = 0

    def method12(self) -> None:
        return None

class NonProto12(Proto12):
    val12: int = 0

    def method12(self) -> None:
        return None

bad_a12: Proto12 = Missing12()
bad_b12: Proto12 = HasVarOnly12()
bad_c12: Proto12 = HasClassVar12()
bad_d12: NonProto12 = HasClassVar12()
bad_e12: NonProto12 = Missing12()
bad_f12: Proto12 = Missing12()

class Proto13(Protocol):
    val13: int

    def method13(self) -> None: ...

class Missing13:
    pass

class HasVarOnly13:
    val13: int = 0

class HasClassVar13:
    val13: ClassVar[int] = 0

    def method13(self) -> None:
        return None

class NonProto13(Proto13):
    val13: int = 0

    def method13(self) -> None:
        return None

bad_a13: Proto13 = Missing13()
bad_b13: Proto13 = HasVarOnly13()
bad_c13: Proto13 = HasClassVar13()
bad_d13: NonProto13 = HasClassVar13()
bad_e13: NonProto13 = Missing13()
bad_f13: Proto13 = Missing13()

class Proto14(Protocol):
    val14: int

    def method14(self) -> None: ...

class Missing14:
    pass

class HasVarOnly14:
    val14: int = 0

class HasClassVar14:
    val14: ClassVar[int] = 0

    def method14(self) -> None:
        return None

class NonProto14(Proto14):
    val14: int = 0

    def method14(self) -> None:
        return None

bad_a14: Proto14 = Missing14()
bad_b14: Proto14 = HasVarOnly14()
bad_c14: Proto14 = HasClassVar14()
bad_d14: NonProto14 = HasClassVar14()
bad_e14: NonProto14 = Missing14()
bad_f14: Proto14 = Missing14()

class Proto15(Protocol):
    val15: int

    def method15(self) -> None: ...

class Missing15:
    pass

class HasVarOnly15:
    val15: int = 0

class HasClassVar15:
    val15: ClassVar[int] = 0

    def method15(self) -> None:
        return None

class NonProto15(Proto15):
    val15: int = 0

    def method15(self) -> None:
        return None

bad_a15: Proto15 = Missing15()
bad_b15: Proto15 = HasVarOnly15()
bad_c15: Proto15 = HasClassVar15()
bad_d15: NonProto15 = HasClassVar15()
bad_e15: NonProto15 = Missing15()
bad_f15: Proto15 = Missing15()

class Proto16(Protocol):
    val16: int

    def method16(self) -> None: ...

class Missing16:
    pass

class HasVarOnly16:
    val16: int = 0

class HasClassVar16:
    val16: ClassVar[int] = 0

    def method16(self) -> None:
        return None

class NonProto16(Proto16):
    val16: int = 0

    def method16(self) -> None:
        return None

bad_a16: Proto16 = Missing16()
bad_b16: Proto16 = HasVarOnly16()
bad_c16: Proto16 = HasClassVar16()
bad_d16: NonProto16 = HasClassVar16()
bad_e16: NonProto16 = Missing16()
bad_f16: Proto16 = Missing16()

class Proto17(Protocol):
    val17: int

    def method17(self) -> None: ...

class Missing17:
    pass

class HasVarOnly17:
    val17: int = 0

class HasClassVar17:
    val17: ClassVar[int] = 0

    def method17(self) -> None:
        return None

class NonProto17(Proto17):
    val17: int = 0

    def method17(self) -> None:
        return None

bad_a17: Proto17 = Missing17()
bad_b17: Proto17 = HasVarOnly17()
bad_c17: Proto17 = HasClassVar17()
bad_d17: NonProto17 = HasClassVar17()
bad_e17: NonProto17 = Missing17()
bad_f17: Proto17 = Missing17()

class Proto18(Protocol):
    val18: int

    def method18(self) -> None: ...

class Missing18:
    pass

class HasVarOnly18:
    val18: int = 0

class HasClassVar18:
    val18: ClassVar[int] = 0

    def method18(self) -> None:
        return None

class NonProto18(Proto18):
    val18: int = 0

    def method18(self) -> None:
        return None

bad_a18: Proto18 = Missing18()
bad_b18: Proto18 = HasVarOnly18()
bad_c18: Proto18 = HasClassVar18()
bad_d18: NonProto18 = HasClassVar18()
bad_e18: NonProto18 = Missing18()
bad_f18: Proto18 = Missing18()

class Proto19(Protocol):
    val19: int

    def method19(self) -> None: ...

class Missing19:
    pass

class HasVarOnly19:
    val19: int = 0

class HasClassVar19:
    val19: ClassVar[int] = 0

    def method19(self) -> None:
        return None

class NonProto19(Proto19):
    val19: int = 0

    def method19(self) -> None:
        return None

bad_a19: Proto19 = Missing19()
bad_b19: Proto19 = HasVarOnly19()
bad_c19: Proto19 = HasClassVar19()
bad_d19: NonProto19 = HasClassVar19()
bad_e19: NonProto19 = Missing19()
bad_f19: Proto19 = Missing19()

class Proto20(Protocol):
    val20: int

    def method20(self) -> None: ...

class Missing20:
    pass

class HasVarOnly20:
    val20: int = 0

class HasClassVar20:
    val20: ClassVar[int] = 0

    def method20(self) -> None:
        return None

class NonProto20(Proto20):
    val20: int = 0

    def method20(self) -> None:
        return None

bad_a20: Proto20 = Missing20()
bad_b20: Proto20 = HasVarOnly20()
bad_c20: Proto20 = HasClassVar20()
bad_d20: NonProto20 = HasClassVar20()
bad_e20: NonProto20 = Missing20()
bad_f20: Proto20 = Missing20()

class Proto21(Protocol):
    val21: int

    def method21(self) -> None: ...

class Missing21:
    pass

class HasVarOnly21:
    val21: int = 0

class HasClassVar21:
    val21: ClassVar[int] = 0

    def method21(self) -> None:
        return None

class NonProto21(Proto21):
    val21: int = 0

    def method21(self) -> None:
        return None

bad_a21: Proto21 = Missing21()
bad_b21: Proto21 = HasVarOnly21()
bad_c21: Proto21 = HasClassVar21()
bad_d21: NonProto21 = HasClassVar21()
bad_e21: NonProto21 = Missing21()
bad_f21: Proto21 = Missing21()

class Proto22(Protocol):
    val22: int

    def method22(self) -> None: ...

class Missing22:
    pass

class HasVarOnly22:
    val22: int = 0

class HasClassVar22:
    val22: ClassVar[int] = 0

    def method22(self) -> None:
        return None

class NonProto22(Proto22):
    val22: int = 0

    def method22(self) -> None:
        return None

bad_a22: Proto22 = Missing22()
bad_b22: Proto22 = HasVarOnly22()
bad_c22: Proto22 = HasClassVar22()
bad_d22: NonProto22 = HasClassVar22()
bad_e22: NonProto22 = Missing22()
bad_f22: Proto22 = Missing22()

class Proto23(Protocol):
    val23: int

    def method23(self) -> None: ...

class Missing23:
    pass

class HasVarOnly23:
    val23: int = 0

class HasClassVar23:
    val23: ClassVar[int] = 0

    def method23(self) -> None:
        return None

class NonProto23(Proto23):
    val23: int = 0

    def method23(self) -> None:
        return None

bad_a23: Proto23 = Missing23()
bad_b23: Proto23 = HasVarOnly23()
bad_c23: Proto23 = HasClassVar23()
bad_d23: NonProto23 = HasClassVar23()
bad_e23: NonProto23 = Missing23()
bad_f23: Proto23 = Missing23()

class Proto24(Protocol):
    val24: int

    def method24(self) -> None: ...

class Missing24:
    pass

class HasVarOnly24:
    val24: int = 0

class HasClassVar24:
    val24: ClassVar[int] = 0

    def method24(self) -> None:
        return None

class NonProto24(Proto24):
    val24: int = 0

    def method24(self) -> None:
        return None

bad_a24: Proto24 = Missing24()
bad_b24: Proto24 = HasVarOnly24()
bad_c24: Proto24 = HasClassVar24()
bad_d24: NonProto24 = HasClassVar24()
bad_e24: NonProto24 = Missing24()
bad_f24: Proto24 = Missing24()

class Proto25(Protocol):
    val25: int

    def method25(self) -> None: ...

class Missing25:
    pass

class HasVarOnly25:
    val25: int = 0

class HasClassVar25:
    val25: ClassVar[int] = 0

    def method25(self) -> None:
        return None

class NonProto25(Proto25):
    val25: int = 0

    def method25(self) -> None:
        return None

bad_a25: Proto25 = Missing25()
bad_b25: Proto25 = HasVarOnly25()
bad_c25: Proto25 = HasClassVar25()
bad_d25: NonProto25 = HasClassVar25()
bad_e25: NonProto25 = Missing25()
bad_f25: Proto25 = Missing25()

class Proto26(Protocol):
    val26: int

    def method26(self) -> None: ...

class Missing26:
    pass

class HasVarOnly26:
    val26: int = 0

class HasClassVar26:
    val26: ClassVar[int] = 0

    def method26(self) -> None:
        return None

class NonProto26(Proto26):
    val26: int = 0

    def method26(self) -> None:
        return None

bad_a26: Proto26 = Missing26()
bad_b26: Proto26 = HasVarOnly26()
bad_c26: Proto26 = HasClassVar26()
bad_d26: NonProto26 = HasClassVar26()
bad_e26: NonProto26 = Missing26()
bad_f26: Proto26 = Missing26()

class Proto27(Protocol):
    val27: int

    def method27(self) -> None: ...

class Missing27:
    pass

class HasVarOnly27:
    val27: int = 0

class HasClassVar27:
    val27: ClassVar[int] = 0

    def method27(self) -> None:
        return None

class NonProto27(Proto27):
    val27: int = 0

    def method27(self) -> None:
        return None

bad_a27: Proto27 = Missing27()
bad_b27: Proto27 = HasVarOnly27()
bad_c27: Proto27 = HasClassVar27()
bad_d27: NonProto27 = HasClassVar27()
bad_e27: NonProto27 = Missing27()
bad_f27: Proto27 = Missing27()

class Proto28(Protocol):
    val28: int

    def method28(self) -> None: ...

class Missing28:
    pass

class HasVarOnly28:
    val28: int = 0

class HasClassVar28:
    val28: ClassVar[int] = 0

    def method28(self) -> None:
        return None

class NonProto28(Proto28):
    val28: int = 0

    def method28(self) -> None:
        return None

bad_a28: Proto28 = Missing28()
bad_b28: Proto28 = HasVarOnly28()
bad_c28: Proto28 = HasClassVar28()
bad_d28: NonProto28 = HasClassVar28()
bad_e28: NonProto28 = Missing28()
bad_f28: Proto28 = Missing28()

class Proto29(Protocol):
    val29: int

    def method29(self) -> None: ...

class Missing29:
    pass

class HasVarOnly29:
    val29: int = 0

class HasClassVar29:
    val29: ClassVar[int] = 0

    def method29(self) -> None:
        return None

class NonProto29(Proto29):
    val29: int = 0

    def method29(self) -> None:
        return None

bad_a29: Proto29 = Missing29()
bad_b29: Proto29 = HasVarOnly29()
bad_c29: Proto29 = HasClassVar29()
bad_d29: NonProto29 = HasClassVar29()
bad_e29: NonProto29 = Missing29()
bad_f29: Proto29 = Missing29()

class Proto30(Protocol):
    val30: int

    def method30(self) -> None: ...

class Missing30:
    pass

class HasVarOnly30:
    val30: int = 0

class HasClassVar30:
    val30: ClassVar[int] = 0

    def method30(self) -> None:
        return None

class NonProto30(Proto30):
    val30: int = 0

    def method30(self) -> None:
        return None

bad_a30: Proto30 = Missing30()
bad_b30: Proto30 = HasVarOnly30()
bad_c30: Proto30 = HasClassVar30()
bad_d30: NonProto30 = HasClassVar30()
bad_e30: NonProto30 = Missing30()
bad_f30: Proto30 = Missing30()

class Proto31(Protocol):
    val31: int

    def method31(self) -> None: ...

class Missing31:
    pass

class HasVarOnly31:
    val31: int = 0

class HasClassVar31:
    val31: ClassVar[int] = 0

    def method31(self) -> None:
        return None

class NonProto31(Proto31):
    val31: int = 0

    def method31(self) -> None:
        return None

bad_a31: Proto31 = Missing31()
bad_b31: Proto31 = HasVarOnly31()
bad_c31: Proto31 = HasClassVar31()
bad_d31: NonProto31 = HasClassVar31()
bad_e31: NonProto31 = Missing31()
bad_f31: Proto31 = Missing31()

class Proto32(Protocol):
    val32: int

    def method32(self) -> None: ...

class Missing32:
    pass

class HasVarOnly32:
    val32: int = 0

class HasClassVar32:
    val32: ClassVar[int] = 0

    def method32(self) -> None:
        return None

class NonProto32(Proto32):
    val32: int = 0

    def method32(self) -> None:
        return None

bad_a32: Proto32 = Missing32()
bad_b32: Proto32 = HasVarOnly32()
bad_c32: Proto32 = HasClassVar32()
bad_d32: NonProto32 = HasClassVar32()
bad_e32: NonProto32 = Missing32()
bad_f32: Proto32 = Missing32()

class Proto33(Protocol):
    val33: int

    def method33(self) -> None: ...

class Missing33:
    pass

class HasVarOnly33:
    val33: int = 0

class HasClassVar33:
    val33: ClassVar[int] = 0

    def method33(self) -> None:
        return None

class NonProto33(Proto33):
    val33: int = 0

    def method33(self) -> None:
        return None

bad_a33: Proto33 = Missing33()
bad_b33: Proto33 = HasVarOnly33()
bad_c33: Proto33 = HasClassVar33()
bad_d33: NonProto33 = HasClassVar33()
bad_e33: NonProto33 = Missing33()
bad_f33: Proto33 = Missing33()

class Proto34(Protocol):
    val34: int

    def method34(self) -> None: ...

class Missing34:
    pass

class HasVarOnly34:
    val34: int = 0

class HasClassVar34:
    val34: ClassVar[int] = 0

    def method34(self) -> None:
        return None

class NonProto34(Proto34):
    val34: int = 0

    def method34(self) -> None:
        return None

bad_a34: Proto34 = Missing34()
bad_b34: Proto34 = HasVarOnly34()
bad_c34: Proto34 = HasClassVar34()
bad_d34: NonProto34 = HasClassVar34()
bad_e34: NonProto34 = Missing34()
bad_f34: Proto34 = Missing34()

class Proto35(Protocol):
    val35: int

    def method35(self) -> None: ...

class Missing35:
    pass

class HasVarOnly35:
    val35: int = 0

class HasClassVar35:
    val35: ClassVar[int] = 0

    def method35(self) -> None:
        return None

class NonProto35(Proto35):
    val35: int = 0

    def method35(self) -> None:
        return None

bad_a35: Proto35 = Missing35()
bad_b35: Proto35 = HasVarOnly35()
bad_c35: Proto35 = HasClassVar35()
bad_d35: NonProto35 = HasClassVar35()
bad_e35: NonProto35 = Missing35()
bad_f35: Proto35 = Missing35()

class Proto36(Protocol):
    val36: int

    def method36(self) -> None: ...

class Missing36:
    pass

class HasVarOnly36:
    val36: int = 0

class HasClassVar36:
    val36: ClassVar[int] = 0

    def method36(self) -> None:
        return None

class NonProto36(Proto36):
    val36: int = 0

    def method36(self) -> None:
        return None

bad_a36: Proto36 = Missing36()
bad_b36: Proto36 = HasVarOnly36()
bad_c36: Proto36 = HasClassVar36()
bad_d36: NonProto36 = HasClassVar36()
bad_e36: NonProto36 = Missing36()
bad_f36: Proto36 = Missing36()

class Proto37(Protocol):
    val37: int

    def method37(self) -> None: ...

class Missing37:
    pass

class HasVarOnly37:
    val37: int = 0

class HasClassVar37:
    val37: ClassVar[int] = 0

    def method37(self) -> None:
        return None

class NonProto37(Proto37):
    val37: int = 0

    def method37(self) -> None:
        return None

bad_a37: Proto37 = Missing37()
bad_b37: Proto37 = HasVarOnly37()
bad_c37: Proto37 = HasClassVar37()
bad_d37: NonProto37 = HasClassVar37()
bad_e37: NonProto37 = Missing37()
bad_f37: Proto37 = Missing37()

class Proto38(Protocol):
    val38: int

    def method38(self) -> None: ...

class Missing38:
    pass

class HasVarOnly38:
    val38: int = 0

class HasClassVar38:
    val38: ClassVar[int] = 0

    def method38(self) -> None:
        return None

class NonProto38(Proto38):
    val38: int = 0

    def method38(self) -> None:
        return None

bad_a38: Proto38 = Missing38()
bad_b38: Proto38 = HasVarOnly38()
bad_c38: Proto38 = HasClassVar38()
bad_d38: NonProto38 = HasClassVar38()
bad_e38: NonProto38 = Missing38()
bad_f38: Proto38 = Missing38()

class Proto39(Protocol):
    val39: int

    def method39(self) -> None: ...

class Missing39:
    pass

class HasVarOnly39:
    val39: int = 0

class HasClassVar39:
    val39: ClassVar[int] = 0

    def method39(self) -> None:
        return None

class NonProto39(Proto39):
    val39: int = 0

    def method39(self) -> None:
        return None

bad_a39: Proto39 = Missing39()
bad_b39: Proto39 = HasVarOnly39()
bad_c39: Proto39 = HasClassVar39()
bad_d39: NonProto39 = HasClassVar39()
bad_e39: NonProto39 = Missing39()
bad_f39: Proto39 = Missing39()

class Proto40(Protocol):
    val40: int

    def method40(self) -> None: ...

class Missing40:
    pass

class HasVarOnly40:
    val40: int = 0

class HasClassVar40:
    val40: ClassVar[int] = 0

    def method40(self) -> None:
        return None

class NonProto40(Proto40):
    val40: int = 0

    def method40(self) -> None:
        return None

bad_a40: Proto40 = Missing40()
bad_b40: Proto40 = HasVarOnly40()
bad_c40: Proto40 = HasClassVar40()
bad_d40: NonProto40 = HasClassVar40()
bad_e40: NonProto40 = Missing40()
bad_f40: Proto40 = Missing40()

class Proto41(Protocol):
    val41: int

    def method41(self) -> None: ...

class Missing41:
    pass

class HasVarOnly41:
    val41: int = 0

class HasClassVar41:
    val41: ClassVar[int] = 0

    def method41(self) -> None:
        return None

class NonProto41(Proto41):
    val41: int = 0

    def method41(self) -> None:
        return None

bad_a41: Proto41 = Missing41()
bad_b41: Proto41 = HasVarOnly41()
bad_c41: Proto41 = HasClassVar41()
bad_d41: NonProto41 = HasClassVar41()
bad_e41: NonProto41 = Missing41()
bad_f41: Proto41 = Missing41()

class Proto42(Protocol):
    val42: int

    def method42(self) -> None: ...

class Missing42:
    pass

class HasVarOnly42:
    val42: int = 0

class HasClassVar42:
    val42: ClassVar[int] = 0

    def method42(self) -> None:
        return None

class NonProto42(Proto42):
    val42: int = 0

    def method42(self) -> None:
        return None

bad_a42: Proto42 = Missing42()
bad_b42: Proto42 = HasVarOnly42()
bad_c42: Proto42 = HasClassVar42()
bad_d42: NonProto42 = HasClassVar42()
bad_e42: NonProto42 = Missing42()
bad_f42: Proto42 = Missing42()

class Proto43(Protocol):
    val43: int

    def method43(self) -> None: ...

class Missing43:
    pass

class HasVarOnly43:
    val43: int = 0

class HasClassVar43:
    val43: ClassVar[int] = 0

    def method43(self) -> None:
        return None

class NonProto43(Proto43):
    val43: int = 0

    def method43(self) -> None:
        return None

bad_a43: Proto43 = Missing43()
bad_b43: Proto43 = HasVarOnly43()
bad_c43: Proto43 = HasClassVar43()
bad_d43: NonProto43 = HasClassVar43()
bad_e43: NonProto43 = Missing43()
bad_f43: Proto43 = Missing43()

class Proto44(Protocol):
    val44: int

    def method44(self) -> None: ...

class Missing44:
    pass

class HasVarOnly44:
    val44: int = 0

class HasClassVar44:
    val44: ClassVar[int] = 0

    def method44(self) -> None:
        return None

class NonProto44(Proto44):
    val44: int = 0

    def method44(self) -> None:
        return None

bad_a44: Proto44 = Missing44()
bad_b44: Proto44 = HasVarOnly44()
bad_c44: Proto44 = HasClassVar44()
bad_d44: NonProto44 = HasClassVar44()
bad_e44: NonProto44 = Missing44()
bad_f44: Proto44 = Missing44()

class Proto45(Protocol):
    val45: int

    def method45(self) -> None: ...

class Missing45:
    pass

class HasVarOnly45:
    val45: int = 0

class HasClassVar45:
    val45: ClassVar[int] = 0

    def method45(self) -> None:
        return None

class NonProto45(Proto45):
    val45: int = 0

    def method45(self) -> None:
        return None

bad_a45: Proto45 = Missing45()
bad_b45: Proto45 = HasVarOnly45()
bad_c45: Proto45 = HasClassVar45()
bad_d45: NonProto45 = HasClassVar45()
bad_e45: NonProto45 = Missing45()
bad_f45: Proto45 = Missing45()

class Proto46(Protocol):
    val46: int

    def method46(self) -> None: ...

class Missing46:
    pass

class HasVarOnly46:
    val46: int = 0

class HasClassVar46:
    val46: ClassVar[int] = 0

    def method46(self) -> None:
        return None

class NonProto46(Proto46):
    val46: int = 0

    def method46(self) -> None:
        return None

bad_a46: Proto46 = Missing46()
bad_b46: Proto46 = HasVarOnly46()
bad_c46: Proto46 = HasClassVar46()
bad_d46: NonProto46 = HasClassVar46()
bad_e46: NonProto46 = Missing46()
bad_f46: Proto46 = Missing46()

class Proto47(Protocol):
    val47: int

    def method47(self) -> None: ...

class Missing47:
    pass

class HasVarOnly47:
    val47: int = 0

class HasClassVar47:
    val47: ClassVar[int] = 0

    def method47(self) -> None:
        return None

class NonProto47(Proto47):
    val47: int = 0

    def method47(self) -> None:
        return None

bad_a47: Proto47 = Missing47()
bad_b47: Proto47 = HasVarOnly47()
bad_c47: Proto47 = HasClassVar47()
bad_d47: NonProto47 = HasClassVar47()
bad_e47: NonProto47 = Missing47()
bad_f47: Proto47 = Missing47()

class Proto48(Protocol):
    val48: int

    def method48(self) -> None: ...

class Missing48:
    pass

class HasVarOnly48:
    val48: int = 0

class HasClassVar48:
    val48: ClassVar[int] = 0

    def method48(self) -> None:
        return None

class NonProto48(Proto48):
    val48: int = 0

    def method48(self) -> None:
        return None

bad_a48: Proto48 = Missing48()
bad_b48: Proto48 = HasVarOnly48()
bad_c48: Proto48 = HasClassVar48()
bad_d48: NonProto48 = HasClassVar48()
bad_e48: NonProto48 = Missing48()
bad_f48: Proto48 = Missing48()

class Proto49(Protocol):
    val49: int

    def method49(self) -> None: ...

class Missing49:
    pass

class HasVarOnly49:
    val49: int = 0

class HasClassVar49:
    val49: ClassVar[int] = 0

    def method49(self) -> None:
        return None

class NonProto49(Proto49):
    val49: int = 0

    def method49(self) -> None:
        return None

bad_a49: Proto49 = Missing49()
bad_b49: Proto49 = HasVarOnly49()
bad_c49: Proto49 = HasClassVar49()
bad_d49: NonProto49 = HasClassVar49()
bad_e49: NonProto49 = Missing49()
bad_f49: Proto49 = Missing49()

class Proto50(Protocol):
    val50: int

    def method50(self) -> None: ...

class Missing50:
    pass

class HasVarOnly50:
    val50: int = 0

class HasClassVar50:
    val50: ClassVar[int] = 0

    def method50(self) -> None:
        return None

class NonProto50(Proto50):
    val50: int = 0

    def method50(self) -> None:
        return None

bad_a50: Proto50 = Missing50()
bad_b50: Proto50 = HasVarOnly50()
bad_c50: Proto50 = HasClassVar50()
bad_d50: NonProto50 = HasClassVar50()
bad_e50: NonProto50 = Missing50()
bad_f50: Proto50 = Missing50()

class Proto51(Protocol):
    val51: int

    def method51(self) -> None: ...

class Missing51:
    pass

class HasVarOnly51:
    val51: int = 0

class HasClassVar51:
    val51: ClassVar[int] = 0

    def method51(self) -> None:
        return None

class NonProto51(Proto51):
    val51: int = 0

    def method51(self) -> None:
        return None

bad_a51: Proto51 = Missing51()
bad_b51: Proto51 = HasVarOnly51()
bad_c51: Proto51 = HasClassVar51()
bad_d51: NonProto51 = HasClassVar51()
bad_e51: NonProto51 = Missing51()
bad_f51: Proto51 = Missing51()

class Proto52(Protocol):
    val52: int

    def method52(self) -> None: ...

class Missing52:
    pass

class HasVarOnly52:
    val52: int = 0

class HasClassVar52:
    val52: ClassVar[int] = 0

    def method52(self) -> None:
        return None

class NonProto52(Proto52):
    val52: int = 0

    def method52(self) -> None:
        return None

bad_a52: Proto52 = Missing52()
bad_b52: Proto52 = HasVarOnly52()
bad_c52: Proto52 = HasClassVar52()
bad_d52: NonProto52 = HasClassVar52()
bad_e52: NonProto52 = Missing52()
bad_f52: Proto52 = Missing52()

class Proto53(Protocol):
    val53: int

    def method53(self) -> None: ...

class Missing53:
    pass

class HasVarOnly53:
    val53: int = 0

class HasClassVar53:
    val53: ClassVar[int] = 0

    def method53(self) -> None:
        return None

class NonProto53(Proto53):
    val53: int = 0

    def method53(self) -> None:
        return None

bad_a53: Proto53 = Missing53()
bad_b53: Proto53 = HasVarOnly53()
bad_c53: Proto53 = HasClassVar53()
bad_d53: NonProto53 = HasClassVar53()
bad_e53: NonProto53 = Missing53()
bad_f53: Proto53 = Missing53()

class Proto54(Protocol):
    val54: int

    def method54(self) -> None: ...

class Missing54:
    pass

class HasVarOnly54:
    val54: int = 0

class HasClassVar54:
    val54: ClassVar[int] = 0

    def method54(self) -> None:
        return None

class NonProto54(Proto54):
    val54: int = 0

    def method54(self) -> None:
        return None

bad_a54: Proto54 = Missing54()
bad_b54: Proto54 = HasVarOnly54()
bad_c54: Proto54 = HasClassVar54()
bad_d54: NonProto54 = HasClassVar54()
bad_e54: NonProto54 = Missing54()
bad_f54: Proto54 = Missing54()

class Proto55(Protocol):
    val55: int

    def method55(self) -> None: ...

class Missing55:
    pass

class HasVarOnly55:
    val55: int = 0

class HasClassVar55:
    val55: ClassVar[int] = 0

    def method55(self) -> None:
        return None

class NonProto55(Proto55):
    val55: int = 0

    def method55(self) -> None:
        return None

bad_a55: Proto55 = Missing55()
bad_b55: Proto55 = HasVarOnly55()
bad_c55: Proto55 = HasClassVar55()
bad_d55: NonProto55 = HasClassVar55()
bad_e55: NonProto55 = Missing55()
bad_f55: Proto55 = Missing55()

class Proto56(Protocol):
    val56: int

    def method56(self) -> None: ...

class Missing56:
    pass

class HasVarOnly56:
    val56: int = 0

class HasClassVar56:
    val56: ClassVar[int] = 0

    def method56(self) -> None:
        return None

class NonProto56(Proto56):
    val56: int = 0

    def method56(self) -> None:
        return None

bad_a56: Proto56 = Missing56()
bad_b56: Proto56 = HasVarOnly56()
bad_c56: Proto56 = HasClassVar56()
bad_d56: NonProto56 = HasClassVar56()
bad_e56: NonProto56 = Missing56()
bad_f56: Proto56 = Missing56()

class Proto57(Protocol):
    val57: int

    def method57(self) -> None: ...

class Missing57:
    pass

class HasVarOnly57:
    val57: int = 0

class HasClassVar57:
    val57: ClassVar[int] = 0

    def method57(self) -> None:
        return None

class NonProto57(Proto57):
    val57: int = 0

    def method57(self) -> None:
        return None

bad_a57: Proto57 = Missing57()
bad_b57: Proto57 = HasVarOnly57()
bad_c57: Proto57 = HasClassVar57()
bad_d57: NonProto57 = HasClassVar57()
bad_e57: NonProto57 = Missing57()
bad_f57: Proto57 = Missing57()

class Proto58(Protocol):
    val58: int

    def method58(self) -> None: ...

class Missing58:
    pass

class HasVarOnly58:
    val58: int = 0

class HasClassVar58:
    val58: ClassVar[int] = 0

    def method58(self) -> None:
        return None

class NonProto58(Proto58):
    val58: int = 0

    def method58(self) -> None:
        return None

bad_a58: Proto58 = Missing58()
bad_b58: Proto58 = HasVarOnly58()
bad_c58: Proto58 = HasClassVar58()
bad_d58: NonProto58 = HasClassVar58()
bad_e58: NonProto58 = Missing58()
bad_f58: Proto58 = Missing58()

class Proto59(Protocol):
    val59: int

    def method59(self) -> None: ...

class Missing59:
    pass

class HasVarOnly59:
    val59: int = 0

class HasClassVar59:
    val59: ClassVar[int] = 0

    def method59(self) -> None:
        return None

class NonProto59(Proto59):
    val59: int = 0

    def method59(self) -> None:
        return None

bad_a59: Proto59 = Missing59()
bad_b59: Proto59 = HasVarOnly59()
bad_c59: Proto59 = HasClassVar59()
bad_d59: NonProto59 = HasClassVar59()
bad_e59: NonProto59 = Missing59()
bad_f59: Proto59 = Missing59()

class Proto60(Protocol):
    val60: int

    def method60(self) -> None: ...

class Missing60:
    pass

class HasVarOnly60:
    val60: int = 0

class HasClassVar60:
    val60: ClassVar[int] = 0

    def method60(self) -> None:
        return None

class NonProto60(Proto60):
    val60: int = 0

    def method60(self) -> None:
        return None

bad_a60: Proto60 = Missing60()
bad_b60: Proto60 = HasVarOnly60()
bad_c60: Proto60 = HasClassVar60()
bad_d60: NonProto60 = HasClassVar60()
bad_e60: NonProto60 = Missing60()
bad_f60: Proto60 = Missing60()

class Proto61(Protocol):
    val61: int

    def method61(self) -> None: ...

class Missing61:
    pass

class HasVarOnly61:
    val61: int = 0

class HasClassVar61:
    val61: ClassVar[int] = 0

    def method61(self) -> None:
        return None

class NonProto61(Proto61):
    val61: int = 0

    def method61(self) -> None:
        return None

bad_a61: Proto61 = Missing61()
bad_b61: Proto61 = HasVarOnly61()
bad_c61: Proto61 = HasClassVar61()
bad_d61: NonProto61 = HasClassVar61()
bad_e61: NonProto61 = Missing61()
bad_f61: Proto61 = Missing61()

class Proto62(Protocol):
    val62: int

    def method62(self) -> None: ...

class Missing62:
    pass

class HasVarOnly62:
    val62: int = 0

class HasClassVar62:
    val62: ClassVar[int] = 0

    def method62(self) -> None:
        return None

class NonProto62(Proto62):
    val62: int = 0

    def method62(self) -> None:
        return None

bad_a62: Proto62 = Missing62()
bad_b62: Proto62 = HasVarOnly62()
bad_c62: Proto62 = HasClassVar62()
bad_d62: NonProto62 = HasClassVar62()
bad_e62: NonProto62 = Missing62()
bad_f62: Proto62 = Missing62()

class Proto63(Protocol):
    val63: int

    def method63(self) -> None: ...

class Missing63:
    pass

class HasVarOnly63:
    val63: int = 0

class HasClassVar63:
    val63: ClassVar[int] = 0

    def method63(self) -> None:
        return None

class NonProto63(Proto63):
    val63: int = 0

    def method63(self) -> None:
        return None

bad_a63: Proto63 = Missing63()
bad_b63: Proto63 = HasVarOnly63()
bad_c63: Proto63 = HasClassVar63()
bad_d63: NonProto63 = HasClassVar63()
bad_e63: NonProto63 = Missing63()
bad_f63: Proto63 = Missing63()

class Proto64(Protocol):
    val64: int

    def method64(self) -> None: ...

class Missing64:
    pass

class HasVarOnly64:
    val64: int = 0

class HasClassVar64:
    val64: ClassVar[int] = 0

    def method64(self) -> None:
        return None

class NonProto64(Proto64):
    val64: int = 0

    def method64(self) -> None:
        return None

bad_a64: Proto64 = Missing64()
bad_b64: Proto64 = HasVarOnly64()
bad_c64: Proto64 = HasClassVar64()
bad_d64: NonProto64 = HasClassVar64()
bad_e64: NonProto64 = Missing64()
bad_f64: Proto64 = Missing64()

class Proto65(Protocol):
    val65: int

    def method65(self) -> None: ...

class Missing65:
    pass

class HasVarOnly65:
    val65: int = 0

class HasClassVar65:
    val65: ClassVar[int] = 0

    def method65(self) -> None:
        return None

class NonProto65(Proto65):
    val65: int = 0

    def method65(self) -> None:
        return None

bad_a65: Proto65 = Missing65()
bad_b65: Proto65 = HasVarOnly65()
bad_c65: Proto65 = HasClassVar65()
bad_d65: NonProto65 = HasClassVar65()
bad_e65: NonProto65 = Missing65()
bad_f65: Proto65 = Missing65()

class Proto66(Protocol):
    val66: int

    def method66(self) -> None: ...

class Missing66:
    pass

class HasVarOnly66:
    val66: int = 0

class HasClassVar66:
    val66: ClassVar[int] = 0

    def method66(self) -> None:
        return None

class NonProto66(Proto66):
    val66: int = 0

    def method66(self) -> None:
        return None

bad_a66: Proto66 = Missing66()
bad_b66: Proto66 = HasVarOnly66()
bad_c66: Proto66 = HasClassVar66()
bad_d66: NonProto66 = HasClassVar66()
bad_e66: NonProto66 = Missing66()
bad_f66: Proto66 = Missing66()

