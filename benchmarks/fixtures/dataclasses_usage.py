"""Benchmark stress fixture for the `dataclasses_usage` rule.

Repeats numbered dataclasses whose `field(default_factory=...)` produces a
type incompatible with the field's declared annotation (PEP 557), padded with
compatible fields so the rule's per-field matching dominates the timing.
"""

from dataclasses import dataclass, field


@dataclass
class Config0:
    bad_a0: int = field(default_factory=str)
    bad_b0: str = field(default_factory=int)
    bad_c0: bytes = field(default_factory=float)
    bad_d0: float | None = field(default_factory=bytes)
    ok_a0: int = field(default_factory=int)
    ok_b0: str = field(default_factory=str)
    ok_c0: float = field(default_factory=int)
    ok_d0: bytes = field(default_factory=bytes)
    ok_e0: list[int] = field(default_factory=list)
    ok_f0: dict[str, int] = field(default_factory=dict)
    ok_g0: set[str] = field(default_factory=set)
    ok_h0: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config1:
    bad_a1: str = field(default_factory=int)
    bad_b1: bytes = field(default_factory=float)
    bad_c1: float | None = field(default_factory=bytes)
    bad_d1: str | None = field(default_factory=bool)
    ok_a1: str = field(default_factory=str)
    ok_b1: float = field(default_factory=int)
    ok_c1: bytes = field(default_factory=bytes)
    ok_d1: list[int] = field(default_factory=list)
    ok_e1: dict[str, int] = field(default_factory=dict)
    ok_f1: set[str] = field(default_factory=set)
    ok_g1: tuple[int, ...] = field(default_factory=tuple)
    ok_h1: int = field(default_factory=int)


@dataclass
class Config2:
    bad_a2: bytes = field(default_factory=float)
    bad_b2: float | None = field(default_factory=bytes)
    bad_c2: str | None = field(default_factory=bool)
    bad_d2: bytes = field(default_factory=int)
    ok_a2: float = field(default_factory=int)
    ok_b2: bytes = field(default_factory=bytes)
    ok_c2: list[int] = field(default_factory=list)
    ok_d2: dict[str, int] = field(default_factory=dict)
    ok_e2: set[str] = field(default_factory=set)
    ok_f2: tuple[int, ...] = field(default_factory=tuple)
    ok_g2: int = field(default_factory=int)
    ok_h2: str = field(default_factory=str)


@dataclass
class Config3:
    bad_a3: float | None = field(default_factory=bytes)
    bad_b3: str | None = field(default_factory=bool)
    bad_c3: bytes = field(default_factory=int)
    bad_d3: float = field(default_factory=str)
    ok_a3: bytes = field(default_factory=bytes)
    ok_b3: list[int] = field(default_factory=list)
    ok_c3: dict[str, int] = field(default_factory=dict)
    ok_d3: set[str] = field(default_factory=set)
    ok_e3: tuple[int, ...] = field(default_factory=tuple)
    ok_f3: int = field(default_factory=int)
    ok_g3: str = field(default_factory=str)
    ok_h3: float = field(default_factory=int)


@dataclass
class Config4:
    bad_a4: str | None = field(default_factory=bool)
    bad_b4: bytes = field(default_factory=int)
    bad_c4: float = field(default_factory=str)
    bad_d4: int = field(default_factory=bytes)
    ok_a4: list[int] = field(default_factory=list)
    ok_b4: dict[str, int] = field(default_factory=dict)
    ok_c4: set[str] = field(default_factory=set)
    ok_d4: tuple[int, ...] = field(default_factory=tuple)
    ok_e4: int = field(default_factory=int)
    ok_f4: str = field(default_factory=str)
    ok_g4: float = field(default_factory=int)
    ok_h4: bytes = field(default_factory=bytes)


@dataclass
class Config5:
    bad_a5: bytes = field(default_factory=int)
    bad_b5: float = field(default_factory=str)
    bad_c5: int = field(default_factory=bytes)
    bad_d5: int = field(default_factory=str)
    ok_a5: dict[str, int] = field(default_factory=dict)
    ok_b5: set[str] = field(default_factory=set)
    ok_c5: tuple[int, ...] = field(default_factory=tuple)
    ok_d5: int = field(default_factory=int)
    ok_e5: str = field(default_factory=str)
    ok_f5: float = field(default_factory=int)
    ok_g5: bytes = field(default_factory=bytes)
    ok_h5: list[int] = field(default_factory=list)


@dataclass
class Config6:
    bad_a6: float = field(default_factory=str)
    bad_b6: int = field(default_factory=bytes)
    bad_c6: int = field(default_factory=str)
    bad_d6: str = field(default_factory=int)
    ok_a6: set[str] = field(default_factory=set)
    ok_b6: tuple[int, ...] = field(default_factory=tuple)
    ok_c6: int = field(default_factory=int)
    ok_d6: str = field(default_factory=str)
    ok_e6: float = field(default_factory=int)
    ok_f6: bytes = field(default_factory=bytes)
    ok_g6: list[int] = field(default_factory=list)
    ok_h6: dict[str, int] = field(default_factory=dict)


@dataclass
class Config7:
    bad_a7: int = field(default_factory=bytes)
    bad_b7: int = field(default_factory=str)
    bad_c7: str = field(default_factory=int)
    bad_d7: bytes = field(default_factory=float)
    ok_a7: tuple[int, ...] = field(default_factory=tuple)
    ok_b7: int = field(default_factory=int)
    ok_c7: str = field(default_factory=str)
    ok_d7: float = field(default_factory=int)
    ok_e7: bytes = field(default_factory=bytes)
    ok_f7: list[int] = field(default_factory=list)
    ok_g7: dict[str, int] = field(default_factory=dict)
    ok_h7: set[str] = field(default_factory=set)


@dataclass
class Config8:
    bad_a8: int = field(default_factory=str)
    bad_b8: str = field(default_factory=int)
    bad_c8: bytes = field(default_factory=float)
    bad_d8: float | None = field(default_factory=bytes)
    ok_a8: int = field(default_factory=int)
    ok_b8: str = field(default_factory=str)
    ok_c8: float = field(default_factory=int)
    ok_d8: bytes = field(default_factory=bytes)
    ok_e8: list[int] = field(default_factory=list)
    ok_f8: dict[str, int] = field(default_factory=dict)
    ok_g8: set[str] = field(default_factory=set)
    ok_h8: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config9:
    bad_a9: str = field(default_factory=int)
    bad_b9: bytes = field(default_factory=float)
    bad_c9: float | None = field(default_factory=bytes)
    bad_d9: str | None = field(default_factory=bool)
    ok_a9: str = field(default_factory=str)
    ok_b9: float = field(default_factory=int)
    ok_c9: bytes = field(default_factory=bytes)
    ok_d9: list[int] = field(default_factory=list)
    ok_e9: dict[str, int] = field(default_factory=dict)
    ok_f9: set[str] = field(default_factory=set)
    ok_g9: tuple[int, ...] = field(default_factory=tuple)
    ok_h9: int = field(default_factory=int)


@dataclass
class Config10:
    bad_a10: bytes = field(default_factory=float)
    bad_b10: float | None = field(default_factory=bytes)
    bad_c10: str | None = field(default_factory=bool)
    bad_d10: bytes = field(default_factory=int)
    ok_a10: float = field(default_factory=int)
    ok_b10: bytes = field(default_factory=bytes)
    ok_c10: list[int] = field(default_factory=list)
    ok_d10: dict[str, int] = field(default_factory=dict)
    ok_e10: set[str] = field(default_factory=set)
    ok_f10: tuple[int, ...] = field(default_factory=tuple)
    ok_g10: int = field(default_factory=int)
    ok_h10: str = field(default_factory=str)


@dataclass
class Config11:
    bad_a11: float | None = field(default_factory=bytes)
    bad_b11: str | None = field(default_factory=bool)
    bad_c11: bytes = field(default_factory=int)
    bad_d11: float = field(default_factory=str)
    ok_a11: bytes = field(default_factory=bytes)
    ok_b11: list[int] = field(default_factory=list)
    ok_c11: dict[str, int] = field(default_factory=dict)
    ok_d11: set[str] = field(default_factory=set)
    ok_e11: tuple[int, ...] = field(default_factory=tuple)
    ok_f11: int = field(default_factory=int)
    ok_g11: str = field(default_factory=str)
    ok_h11: float = field(default_factory=int)


@dataclass
class Config12:
    bad_a12: str | None = field(default_factory=bool)
    bad_b12: bytes = field(default_factory=int)
    bad_c12: float = field(default_factory=str)
    bad_d12: int = field(default_factory=bytes)
    ok_a12: list[int] = field(default_factory=list)
    ok_b12: dict[str, int] = field(default_factory=dict)
    ok_c12: set[str] = field(default_factory=set)
    ok_d12: tuple[int, ...] = field(default_factory=tuple)
    ok_e12: int = field(default_factory=int)
    ok_f12: str = field(default_factory=str)
    ok_g12: float = field(default_factory=int)
    ok_h12: bytes = field(default_factory=bytes)


@dataclass
class Config13:
    bad_a13: bytes = field(default_factory=int)
    bad_b13: float = field(default_factory=str)
    bad_c13: int = field(default_factory=bytes)
    bad_d13: int = field(default_factory=str)
    ok_a13: dict[str, int] = field(default_factory=dict)
    ok_b13: set[str] = field(default_factory=set)
    ok_c13: tuple[int, ...] = field(default_factory=tuple)
    ok_d13: int = field(default_factory=int)
    ok_e13: str = field(default_factory=str)
    ok_f13: float = field(default_factory=int)
    ok_g13: bytes = field(default_factory=bytes)
    ok_h13: list[int] = field(default_factory=list)


@dataclass
class Config14:
    bad_a14: float = field(default_factory=str)
    bad_b14: int = field(default_factory=bytes)
    bad_c14: int = field(default_factory=str)
    bad_d14: str = field(default_factory=int)
    ok_a14: set[str] = field(default_factory=set)
    ok_b14: tuple[int, ...] = field(default_factory=tuple)
    ok_c14: int = field(default_factory=int)
    ok_d14: str = field(default_factory=str)
    ok_e14: float = field(default_factory=int)
    ok_f14: bytes = field(default_factory=bytes)
    ok_g14: list[int] = field(default_factory=list)
    ok_h14: dict[str, int] = field(default_factory=dict)


@dataclass
class Config15:
    bad_a15: int = field(default_factory=bytes)
    bad_b15: int = field(default_factory=str)
    bad_c15: str = field(default_factory=int)
    bad_d15: bytes = field(default_factory=float)
    ok_a15: tuple[int, ...] = field(default_factory=tuple)
    ok_b15: int = field(default_factory=int)
    ok_c15: str = field(default_factory=str)
    ok_d15: float = field(default_factory=int)
    ok_e15: bytes = field(default_factory=bytes)
    ok_f15: list[int] = field(default_factory=list)
    ok_g15: dict[str, int] = field(default_factory=dict)
    ok_h15: set[str] = field(default_factory=set)


@dataclass
class Config16:
    bad_a16: int = field(default_factory=str)
    bad_b16: str = field(default_factory=int)
    bad_c16: bytes = field(default_factory=float)
    bad_d16: float | None = field(default_factory=bytes)
    ok_a16: int = field(default_factory=int)
    ok_b16: str = field(default_factory=str)
    ok_c16: float = field(default_factory=int)
    ok_d16: bytes = field(default_factory=bytes)
    ok_e16: list[int] = field(default_factory=list)
    ok_f16: dict[str, int] = field(default_factory=dict)
    ok_g16: set[str] = field(default_factory=set)
    ok_h16: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config17:
    bad_a17: str = field(default_factory=int)
    bad_b17: bytes = field(default_factory=float)
    bad_c17: float | None = field(default_factory=bytes)
    bad_d17: str | None = field(default_factory=bool)
    ok_a17: str = field(default_factory=str)
    ok_b17: float = field(default_factory=int)
    ok_c17: bytes = field(default_factory=bytes)
    ok_d17: list[int] = field(default_factory=list)
    ok_e17: dict[str, int] = field(default_factory=dict)
    ok_f17: set[str] = field(default_factory=set)
    ok_g17: tuple[int, ...] = field(default_factory=tuple)
    ok_h17: int = field(default_factory=int)


@dataclass
class Config18:
    bad_a18: bytes = field(default_factory=float)
    bad_b18: float | None = field(default_factory=bytes)
    bad_c18: str | None = field(default_factory=bool)
    bad_d18: bytes = field(default_factory=int)
    ok_a18: float = field(default_factory=int)
    ok_b18: bytes = field(default_factory=bytes)
    ok_c18: list[int] = field(default_factory=list)
    ok_d18: dict[str, int] = field(default_factory=dict)
    ok_e18: set[str] = field(default_factory=set)
    ok_f18: tuple[int, ...] = field(default_factory=tuple)
    ok_g18: int = field(default_factory=int)
    ok_h18: str = field(default_factory=str)


@dataclass
class Config19:
    bad_a19: float | None = field(default_factory=bytes)
    bad_b19: str | None = field(default_factory=bool)
    bad_c19: bytes = field(default_factory=int)
    bad_d19: float = field(default_factory=str)
    ok_a19: bytes = field(default_factory=bytes)
    ok_b19: list[int] = field(default_factory=list)
    ok_c19: dict[str, int] = field(default_factory=dict)
    ok_d19: set[str] = field(default_factory=set)
    ok_e19: tuple[int, ...] = field(default_factory=tuple)
    ok_f19: int = field(default_factory=int)
    ok_g19: str = field(default_factory=str)
    ok_h19: float = field(default_factory=int)


@dataclass
class Config20:
    bad_a20: str | None = field(default_factory=bool)
    bad_b20: bytes = field(default_factory=int)
    bad_c20: float = field(default_factory=str)
    bad_d20: int = field(default_factory=bytes)
    ok_a20: list[int] = field(default_factory=list)
    ok_b20: dict[str, int] = field(default_factory=dict)
    ok_c20: set[str] = field(default_factory=set)
    ok_d20: tuple[int, ...] = field(default_factory=tuple)
    ok_e20: int = field(default_factory=int)
    ok_f20: str = field(default_factory=str)
    ok_g20: float = field(default_factory=int)
    ok_h20: bytes = field(default_factory=bytes)


@dataclass
class Config21:
    bad_a21: bytes = field(default_factory=int)
    bad_b21: float = field(default_factory=str)
    bad_c21: int = field(default_factory=bytes)
    bad_d21: int = field(default_factory=str)
    ok_a21: dict[str, int] = field(default_factory=dict)
    ok_b21: set[str] = field(default_factory=set)
    ok_c21: tuple[int, ...] = field(default_factory=tuple)
    ok_d21: int = field(default_factory=int)
    ok_e21: str = field(default_factory=str)
    ok_f21: float = field(default_factory=int)
    ok_g21: bytes = field(default_factory=bytes)
    ok_h21: list[int] = field(default_factory=list)


@dataclass
class Config22:
    bad_a22: float = field(default_factory=str)
    bad_b22: int = field(default_factory=bytes)
    bad_c22: int = field(default_factory=str)
    bad_d22: str = field(default_factory=int)
    ok_a22: set[str] = field(default_factory=set)
    ok_b22: tuple[int, ...] = field(default_factory=tuple)
    ok_c22: int = field(default_factory=int)
    ok_d22: str = field(default_factory=str)
    ok_e22: float = field(default_factory=int)
    ok_f22: bytes = field(default_factory=bytes)
    ok_g22: list[int] = field(default_factory=list)
    ok_h22: dict[str, int] = field(default_factory=dict)


@dataclass
class Config23:
    bad_a23: int = field(default_factory=bytes)
    bad_b23: int = field(default_factory=str)
    bad_c23: str = field(default_factory=int)
    bad_d23: bytes = field(default_factory=float)
    ok_a23: tuple[int, ...] = field(default_factory=tuple)
    ok_b23: int = field(default_factory=int)
    ok_c23: str = field(default_factory=str)
    ok_d23: float = field(default_factory=int)
    ok_e23: bytes = field(default_factory=bytes)
    ok_f23: list[int] = field(default_factory=list)
    ok_g23: dict[str, int] = field(default_factory=dict)
    ok_h23: set[str] = field(default_factory=set)


@dataclass
class Config24:
    bad_a24: int = field(default_factory=str)
    bad_b24: str = field(default_factory=int)
    bad_c24: bytes = field(default_factory=float)
    bad_d24: float | None = field(default_factory=bytes)
    ok_a24: int = field(default_factory=int)
    ok_b24: str = field(default_factory=str)
    ok_c24: float = field(default_factory=int)
    ok_d24: bytes = field(default_factory=bytes)
    ok_e24: list[int] = field(default_factory=list)
    ok_f24: dict[str, int] = field(default_factory=dict)
    ok_g24: set[str] = field(default_factory=set)
    ok_h24: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config25:
    bad_a25: str = field(default_factory=int)
    bad_b25: bytes = field(default_factory=float)
    bad_c25: float | None = field(default_factory=bytes)
    bad_d25: str | None = field(default_factory=bool)
    ok_a25: str = field(default_factory=str)
    ok_b25: float = field(default_factory=int)
    ok_c25: bytes = field(default_factory=bytes)
    ok_d25: list[int] = field(default_factory=list)
    ok_e25: dict[str, int] = field(default_factory=dict)
    ok_f25: set[str] = field(default_factory=set)
    ok_g25: tuple[int, ...] = field(default_factory=tuple)
    ok_h25: int = field(default_factory=int)


@dataclass
class Config26:
    bad_a26: bytes = field(default_factory=float)
    bad_b26: float | None = field(default_factory=bytes)
    bad_c26: str | None = field(default_factory=bool)
    bad_d26: bytes = field(default_factory=int)
    ok_a26: float = field(default_factory=int)
    ok_b26: bytes = field(default_factory=bytes)
    ok_c26: list[int] = field(default_factory=list)
    ok_d26: dict[str, int] = field(default_factory=dict)
    ok_e26: set[str] = field(default_factory=set)
    ok_f26: tuple[int, ...] = field(default_factory=tuple)
    ok_g26: int = field(default_factory=int)
    ok_h26: str = field(default_factory=str)


@dataclass
class Config27:
    bad_a27: float | None = field(default_factory=bytes)
    bad_b27: str | None = field(default_factory=bool)
    bad_c27: bytes = field(default_factory=int)
    bad_d27: float = field(default_factory=str)
    ok_a27: bytes = field(default_factory=bytes)
    ok_b27: list[int] = field(default_factory=list)
    ok_c27: dict[str, int] = field(default_factory=dict)
    ok_d27: set[str] = field(default_factory=set)
    ok_e27: tuple[int, ...] = field(default_factory=tuple)
    ok_f27: int = field(default_factory=int)
    ok_g27: str = field(default_factory=str)
    ok_h27: float = field(default_factory=int)


@dataclass
class Config28:
    bad_a28: str | None = field(default_factory=bool)
    bad_b28: bytes = field(default_factory=int)
    bad_c28: float = field(default_factory=str)
    bad_d28: int = field(default_factory=bytes)
    ok_a28: list[int] = field(default_factory=list)
    ok_b28: dict[str, int] = field(default_factory=dict)
    ok_c28: set[str] = field(default_factory=set)
    ok_d28: tuple[int, ...] = field(default_factory=tuple)
    ok_e28: int = field(default_factory=int)
    ok_f28: str = field(default_factory=str)
    ok_g28: float = field(default_factory=int)
    ok_h28: bytes = field(default_factory=bytes)


@dataclass
class Config29:
    bad_a29: bytes = field(default_factory=int)
    bad_b29: float = field(default_factory=str)
    bad_c29: int = field(default_factory=bytes)
    bad_d29: int = field(default_factory=str)
    ok_a29: dict[str, int] = field(default_factory=dict)
    ok_b29: set[str] = field(default_factory=set)
    ok_c29: tuple[int, ...] = field(default_factory=tuple)
    ok_d29: int = field(default_factory=int)
    ok_e29: str = field(default_factory=str)
    ok_f29: float = field(default_factory=int)
    ok_g29: bytes = field(default_factory=bytes)
    ok_h29: list[int] = field(default_factory=list)


@dataclass
class Config30:
    bad_a30: float = field(default_factory=str)
    bad_b30: int = field(default_factory=bytes)
    bad_c30: int = field(default_factory=str)
    bad_d30: str = field(default_factory=int)
    ok_a30: set[str] = field(default_factory=set)
    ok_b30: tuple[int, ...] = field(default_factory=tuple)
    ok_c30: int = field(default_factory=int)
    ok_d30: str = field(default_factory=str)
    ok_e30: float = field(default_factory=int)
    ok_f30: bytes = field(default_factory=bytes)
    ok_g30: list[int] = field(default_factory=list)
    ok_h30: dict[str, int] = field(default_factory=dict)


@dataclass
class Config31:
    bad_a31: int = field(default_factory=bytes)
    bad_b31: int = field(default_factory=str)
    bad_c31: str = field(default_factory=int)
    bad_d31: bytes = field(default_factory=float)
    ok_a31: tuple[int, ...] = field(default_factory=tuple)
    ok_b31: int = field(default_factory=int)
    ok_c31: str = field(default_factory=str)
    ok_d31: float = field(default_factory=int)
    ok_e31: bytes = field(default_factory=bytes)
    ok_f31: list[int] = field(default_factory=list)
    ok_g31: dict[str, int] = field(default_factory=dict)
    ok_h31: set[str] = field(default_factory=set)


@dataclass
class Config32:
    bad_a32: int = field(default_factory=str)
    bad_b32: str = field(default_factory=int)
    bad_c32: bytes = field(default_factory=float)
    bad_d32: float | None = field(default_factory=bytes)
    ok_a32: int = field(default_factory=int)
    ok_b32: str = field(default_factory=str)
    ok_c32: float = field(default_factory=int)
    ok_d32: bytes = field(default_factory=bytes)
    ok_e32: list[int] = field(default_factory=list)
    ok_f32: dict[str, int] = field(default_factory=dict)
    ok_g32: set[str] = field(default_factory=set)
    ok_h32: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config33:
    bad_a33: str = field(default_factory=int)
    bad_b33: bytes = field(default_factory=float)
    bad_c33: float | None = field(default_factory=bytes)
    bad_d33: str | None = field(default_factory=bool)
    ok_a33: str = field(default_factory=str)
    ok_b33: float = field(default_factory=int)
    ok_c33: bytes = field(default_factory=bytes)
    ok_d33: list[int] = field(default_factory=list)
    ok_e33: dict[str, int] = field(default_factory=dict)
    ok_f33: set[str] = field(default_factory=set)
    ok_g33: tuple[int, ...] = field(default_factory=tuple)
    ok_h33: int = field(default_factory=int)


@dataclass
class Config34:
    bad_a34: bytes = field(default_factory=float)
    bad_b34: float | None = field(default_factory=bytes)
    bad_c34: str | None = field(default_factory=bool)
    bad_d34: bytes = field(default_factory=int)
    ok_a34: float = field(default_factory=int)
    ok_b34: bytes = field(default_factory=bytes)
    ok_c34: list[int] = field(default_factory=list)
    ok_d34: dict[str, int] = field(default_factory=dict)
    ok_e34: set[str] = field(default_factory=set)
    ok_f34: tuple[int, ...] = field(default_factory=tuple)
    ok_g34: int = field(default_factory=int)
    ok_h34: str = field(default_factory=str)


@dataclass
class Config35:
    bad_a35: float | None = field(default_factory=bytes)
    bad_b35: str | None = field(default_factory=bool)
    bad_c35: bytes = field(default_factory=int)
    bad_d35: float = field(default_factory=str)
    ok_a35: bytes = field(default_factory=bytes)
    ok_b35: list[int] = field(default_factory=list)
    ok_c35: dict[str, int] = field(default_factory=dict)
    ok_d35: set[str] = field(default_factory=set)
    ok_e35: tuple[int, ...] = field(default_factory=tuple)
    ok_f35: int = field(default_factory=int)
    ok_g35: str = field(default_factory=str)
    ok_h35: float = field(default_factory=int)


@dataclass
class Config36:
    bad_a36: str | None = field(default_factory=bool)
    bad_b36: bytes = field(default_factory=int)
    bad_c36: float = field(default_factory=str)
    bad_d36: int = field(default_factory=bytes)
    ok_a36: list[int] = field(default_factory=list)
    ok_b36: dict[str, int] = field(default_factory=dict)
    ok_c36: set[str] = field(default_factory=set)
    ok_d36: tuple[int, ...] = field(default_factory=tuple)
    ok_e36: int = field(default_factory=int)
    ok_f36: str = field(default_factory=str)
    ok_g36: float = field(default_factory=int)
    ok_h36: bytes = field(default_factory=bytes)


@dataclass
class Config37:
    bad_a37: bytes = field(default_factory=int)
    bad_b37: float = field(default_factory=str)
    bad_c37: int = field(default_factory=bytes)
    bad_d37: int = field(default_factory=str)
    ok_a37: dict[str, int] = field(default_factory=dict)
    ok_b37: set[str] = field(default_factory=set)
    ok_c37: tuple[int, ...] = field(default_factory=tuple)
    ok_d37: int = field(default_factory=int)
    ok_e37: str = field(default_factory=str)
    ok_f37: float = field(default_factory=int)
    ok_g37: bytes = field(default_factory=bytes)
    ok_h37: list[int] = field(default_factory=list)


@dataclass
class Config38:
    bad_a38: float = field(default_factory=str)
    bad_b38: int = field(default_factory=bytes)
    bad_c38: int = field(default_factory=str)
    bad_d38: str = field(default_factory=int)
    ok_a38: set[str] = field(default_factory=set)
    ok_b38: tuple[int, ...] = field(default_factory=tuple)
    ok_c38: int = field(default_factory=int)
    ok_d38: str = field(default_factory=str)
    ok_e38: float = field(default_factory=int)
    ok_f38: bytes = field(default_factory=bytes)
    ok_g38: list[int] = field(default_factory=list)
    ok_h38: dict[str, int] = field(default_factory=dict)


@dataclass
class Config39:
    bad_a39: int = field(default_factory=bytes)
    bad_b39: int = field(default_factory=str)
    bad_c39: str = field(default_factory=int)
    bad_d39: bytes = field(default_factory=float)
    ok_a39: tuple[int, ...] = field(default_factory=tuple)
    ok_b39: int = field(default_factory=int)
    ok_c39: str = field(default_factory=str)
    ok_d39: float = field(default_factory=int)
    ok_e39: bytes = field(default_factory=bytes)
    ok_f39: list[int] = field(default_factory=list)
    ok_g39: dict[str, int] = field(default_factory=dict)
    ok_h39: set[str] = field(default_factory=set)


@dataclass
class Config40:
    bad_a40: int = field(default_factory=str)
    bad_b40: str = field(default_factory=int)
    bad_c40: bytes = field(default_factory=float)
    bad_d40: float | None = field(default_factory=bytes)
    ok_a40: int = field(default_factory=int)
    ok_b40: str = field(default_factory=str)
    ok_c40: float = field(default_factory=int)
    ok_d40: bytes = field(default_factory=bytes)
    ok_e40: list[int] = field(default_factory=list)
    ok_f40: dict[str, int] = field(default_factory=dict)
    ok_g40: set[str] = field(default_factory=set)
    ok_h40: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config41:
    bad_a41: str = field(default_factory=int)
    bad_b41: bytes = field(default_factory=float)
    bad_c41: float | None = field(default_factory=bytes)
    bad_d41: str | None = field(default_factory=bool)
    ok_a41: str = field(default_factory=str)
    ok_b41: float = field(default_factory=int)
    ok_c41: bytes = field(default_factory=bytes)
    ok_d41: list[int] = field(default_factory=list)
    ok_e41: dict[str, int] = field(default_factory=dict)
    ok_f41: set[str] = field(default_factory=set)
    ok_g41: tuple[int, ...] = field(default_factory=tuple)
    ok_h41: int = field(default_factory=int)


@dataclass
class Config42:
    bad_a42: bytes = field(default_factory=float)
    bad_b42: float | None = field(default_factory=bytes)
    bad_c42: str | None = field(default_factory=bool)
    bad_d42: bytes = field(default_factory=int)
    ok_a42: float = field(default_factory=int)
    ok_b42: bytes = field(default_factory=bytes)
    ok_c42: list[int] = field(default_factory=list)
    ok_d42: dict[str, int] = field(default_factory=dict)
    ok_e42: set[str] = field(default_factory=set)
    ok_f42: tuple[int, ...] = field(default_factory=tuple)
    ok_g42: int = field(default_factory=int)
    ok_h42: str = field(default_factory=str)


@dataclass
class Config43:
    bad_a43: float | None = field(default_factory=bytes)
    bad_b43: str | None = field(default_factory=bool)
    bad_c43: bytes = field(default_factory=int)
    bad_d43: float = field(default_factory=str)
    ok_a43: bytes = field(default_factory=bytes)
    ok_b43: list[int] = field(default_factory=list)
    ok_c43: dict[str, int] = field(default_factory=dict)
    ok_d43: set[str] = field(default_factory=set)
    ok_e43: tuple[int, ...] = field(default_factory=tuple)
    ok_f43: int = field(default_factory=int)
    ok_g43: str = field(default_factory=str)
    ok_h43: float = field(default_factory=int)


@dataclass
class Config44:
    bad_a44: str | None = field(default_factory=bool)
    bad_b44: bytes = field(default_factory=int)
    bad_c44: float = field(default_factory=str)
    bad_d44: int = field(default_factory=bytes)
    ok_a44: list[int] = field(default_factory=list)
    ok_b44: dict[str, int] = field(default_factory=dict)
    ok_c44: set[str] = field(default_factory=set)
    ok_d44: tuple[int, ...] = field(default_factory=tuple)
    ok_e44: int = field(default_factory=int)
    ok_f44: str = field(default_factory=str)
    ok_g44: float = field(default_factory=int)
    ok_h44: bytes = field(default_factory=bytes)


@dataclass
class Config45:
    bad_a45: bytes = field(default_factory=int)
    bad_b45: float = field(default_factory=str)
    bad_c45: int = field(default_factory=bytes)
    bad_d45: int = field(default_factory=str)
    ok_a45: dict[str, int] = field(default_factory=dict)
    ok_b45: set[str] = field(default_factory=set)
    ok_c45: tuple[int, ...] = field(default_factory=tuple)
    ok_d45: int = field(default_factory=int)
    ok_e45: str = field(default_factory=str)
    ok_f45: float = field(default_factory=int)
    ok_g45: bytes = field(default_factory=bytes)
    ok_h45: list[int] = field(default_factory=list)


@dataclass
class Config46:
    bad_a46: float = field(default_factory=str)
    bad_b46: int = field(default_factory=bytes)
    bad_c46: int = field(default_factory=str)
    bad_d46: str = field(default_factory=int)
    ok_a46: set[str] = field(default_factory=set)
    ok_b46: tuple[int, ...] = field(default_factory=tuple)
    ok_c46: int = field(default_factory=int)
    ok_d46: str = field(default_factory=str)
    ok_e46: float = field(default_factory=int)
    ok_f46: bytes = field(default_factory=bytes)
    ok_g46: list[int] = field(default_factory=list)
    ok_h46: dict[str, int] = field(default_factory=dict)


@dataclass
class Config47:
    bad_a47: int = field(default_factory=bytes)
    bad_b47: int = field(default_factory=str)
    bad_c47: str = field(default_factory=int)
    bad_d47: bytes = field(default_factory=float)
    ok_a47: tuple[int, ...] = field(default_factory=tuple)
    ok_b47: int = field(default_factory=int)
    ok_c47: str = field(default_factory=str)
    ok_d47: float = field(default_factory=int)
    ok_e47: bytes = field(default_factory=bytes)
    ok_f47: list[int] = field(default_factory=list)
    ok_g47: dict[str, int] = field(default_factory=dict)
    ok_h47: set[str] = field(default_factory=set)


@dataclass
class Config48:
    bad_a48: int = field(default_factory=str)
    bad_b48: str = field(default_factory=int)
    bad_c48: bytes = field(default_factory=float)
    bad_d48: float | None = field(default_factory=bytes)
    ok_a48: int = field(default_factory=int)
    ok_b48: str = field(default_factory=str)
    ok_c48: float = field(default_factory=int)
    ok_d48: bytes = field(default_factory=bytes)
    ok_e48: list[int] = field(default_factory=list)
    ok_f48: dict[str, int] = field(default_factory=dict)
    ok_g48: set[str] = field(default_factory=set)
    ok_h48: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config49:
    bad_a49: str = field(default_factory=int)
    bad_b49: bytes = field(default_factory=float)
    bad_c49: float | None = field(default_factory=bytes)
    bad_d49: str | None = field(default_factory=bool)
    ok_a49: str = field(default_factory=str)
    ok_b49: float = field(default_factory=int)
    ok_c49: bytes = field(default_factory=bytes)
    ok_d49: list[int] = field(default_factory=list)
    ok_e49: dict[str, int] = field(default_factory=dict)
    ok_f49: set[str] = field(default_factory=set)
    ok_g49: tuple[int, ...] = field(default_factory=tuple)
    ok_h49: int = field(default_factory=int)


@dataclass
class Config50:
    bad_a50: bytes = field(default_factory=float)
    bad_b50: float | None = field(default_factory=bytes)
    bad_c50: str | None = field(default_factory=bool)
    bad_d50: bytes = field(default_factory=int)
    ok_a50: float = field(default_factory=int)
    ok_b50: bytes = field(default_factory=bytes)
    ok_c50: list[int] = field(default_factory=list)
    ok_d50: dict[str, int] = field(default_factory=dict)
    ok_e50: set[str] = field(default_factory=set)
    ok_f50: tuple[int, ...] = field(default_factory=tuple)
    ok_g50: int = field(default_factory=int)
    ok_h50: str = field(default_factory=str)


@dataclass
class Config51:
    bad_a51: float | None = field(default_factory=bytes)
    bad_b51: str | None = field(default_factory=bool)
    bad_c51: bytes = field(default_factory=int)
    bad_d51: float = field(default_factory=str)
    ok_a51: bytes = field(default_factory=bytes)
    ok_b51: list[int] = field(default_factory=list)
    ok_c51: dict[str, int] = field(default_factory=dict)
    ok_d51: set[str] = field(default_factory=set)
    ok_e51: tuple[int, ...] = field(default_factory=tuple)
    ok_f51: int = field(default_factory=int)
    ok_g51: str = field(default_factory=str)
    ok_h51: float = field(default_factory=int)


@dataclass
class Config52:
    bad_a52: str | None = field(default_factory=bool)
    bad_b52: bytes = field(default_factory=int)
    bad_c52: float = field(default_factory=str)
    bad_d52: int = field(default_factory=bytes)
    ok_a52: list[int] = field(default_factory=list)
    ok_b52: dict[str, int] = field(default_factory=dict)
    ok_c52: set[str] = field(default_factory=set)
    ok_d52: tuple[int, ...] = field(default_factory=tuple)
    ok_e52: int = field(default_factory=int)
    ok_f52: str = field(default_factory=str)
    ok_g52: float = field(default_factory=int)
    ok_h52: bytes = field(default_factory=bytes)


@dataclass
class Config53:
    bad_a53: bytes = field(default_factory=int)
    bad_b53: float = field(default_factory=str)
    bad_c53: int = field(default_factory=bytes)
    bad_d53: int = field(default_factory=str)
    ok_a53: dict[str, int] = field(default_factory=dict)
    ok_b53: set[str] = field(default_factory=set)
    ok_c53: tuple[int, ...] = field(default_factory=tuple)
    ok_d53: int = field(default_factory=int)
    ok_e53: str = field(default_factory=str)
    ok_f53: float = field(default_factory=int)
    ok_g53: bytes = field(default_factory=bytes)
    ok_h53: list[int] = field(default_factory=list)


@dataclass
class Config54:
    bad_a54: float = field(default_factory=str)
    bad_b54: int = field(default_factory=bytes)
    bad_c54: int = field(default_factory=str)
    bad_d54: str = field(default_factory=int)
    ok_a54: set[str] = field(default_factory=set)
    ok_b54: tuple[int, ...] = field(default_factory=tuple)
    ok_c54: int = field(default_factory=int)
    ok_d54: str = field(default_factory=str)
    ok_e54: float = field(default_factory=int)
    ok_f54: bytes = field(default_factory=bytes)
    ok_g54: list[int] = field(default_factory=list)
    ok_h54: dict[str, int] = field(default_factory=dict)


@dataclass
class Config55:
    bad_a55: int = field(default_factory=bytes)
    bad_b55: int = field(default_factory=str)
    bad_c55: str = field(default_factory=int)
    bad_d55: bytes = field(default_factory=float)
    ok_a55: tuple[int, ...] = field(default_factory=tuple)
    ok_b55: int = field(default_factory=int)
    ok_c55: str = field(default_factory=str)
    ok_d55: float = field(default_factory=int)
    ok_e55: bytes = field(default_factory=bytes)
    ok_f55: list[int] = field(default_factory=list)
    ok_g55: dict[str, int] = field(default_factory=dict)
    ok_h55: set[str] = field(default_factory=set)


@dataclass
class Config56:
    bad_a56: int = field(default_factory=str)
    bad_b56: str = field(default_factory=int)
    bad_c56: bytes = field(default_factory=float)
    bad_d56: float | None = field(default_factory=bytes)
    ok_a56: int = field(default_factory=int)
    ok_b56: str = field(default_factory=str)
    ok_c56: float = field(default_factory=int)
    ok_d56: bytes = field(default_factory=bytes)
    ok_e56: list[int] = field(default_factory=list)
    ok_f56: dict[str, int] = field(default_factory=dict)
    ok_g56: set[str] = field(default_factory=set)
    ok_h56: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config57:
    bad_a57: str = field(default_factory=int)
    bad_b57: bytes = field(default_factory=float)
    bad_c57: float | None = field(default_factory=bytes)
    bad_d57: str | None = field(default_factory=bool)
    ok_a57: str = field(default_factory=str)
    ok_b57: float = field(default_factory=int)
    ok_c57: bytes = field(default_factory=bytes)
    ok_d57: list[int] = field(default_factory=list)
    ok_e57: dict[str, int] = field(default_factory=dict)
    ok_f57: set[str] = field(default_factory=set)
    ok_g57: tuple[int, ...] = field(default_factory=tuple)
    ok_h57: int = field(default_factory=int)


@dataclass
class Config58:
    bad_a58: bytes = field(default_factory=float)
    bad_b58: float | None = field(default_factory=bytes)
    bad_c58: str | None = field(default_factory=bool)
    bad_d58: bytes = field(default_factory=int)
    ok_a58: float = field(default_factory=int)
    ok_b58: bytes = field(default_factory=bytes)
    ok_c58: list[int] = field(default_factory=list)
    ok_d58: dict[str, int] = field(default_factory=dict)
    ok_e58: set[str] = field(default_factory=set)
    ok_f58: tuple[int, ...] = field(default_factory=tuple)
    ok_g58: int = field(default_factory=int)
    ok_h58: str = field(default_factory=str)


@dataclass
class Config59:
    bad_a59: float | None = field(default_factory=bytes)
    bad_b59: str | None = field(default_factory=bool)
    bad_c59: bytes = field(default_factory=int)
    bad_d59: float = field(default_factory=str)
    ok_a59: bytes = field(default_factory=bytes)
    ok_b59: list[int] = field(default_factory=list)
    ok_c59: dict[str, int] = field(default_factory=dict)
    ok_d59: set[str] = field(default_factory=set)
    ok_e59: tuple[int, ...] = field(default_factory=tuple)
    ok_f59: int = field(default_factory=int)
    ok_g59: str = field(default_factory=str)
    ok_h59: float = field(default_factory=int)


@dataclass
class Config60:
    bad_a60: str | None = field(default_factory=bool)
    bad_b60: bytes = field(default_factory=int)
    bad_c60: float = field(default_factory=str)
    bad_d60: int = field(default_factory=bytes)
    ok_a60: list[int] = field(default_factory=list)
    ok_b60: dict[str, int] = field(default_factory=dict)
    ok_c60: set[str] = field(default_factory=set)
    ok_d60: tuple[int, ...] = field(default_factory=tuple)
    ok_e60: int = field(default_factory=int)
    ok_f60: str = field(default_factory=str)
    ok_g60: float = field(default_factory=int)
    ok_h60: bytes = field(default_factory=bytes)


@dataclass
class Config61:
    bad_a61: bytes = field(default_factory=int)
    bad_b61: float = field(default_factory=str)
    bad_c61: int = field(default_factory=bytes)
    bad_d61: int = field(default_factory=str)
    ok_a61: dict[str, int] = field(default_factory=dict)
    ok_b61: set[str] = field(default_factory=set)
    ok_c61: tuple[int, ...] = field(default_factory=tuple)
    ok_d61: int = field(default_factory=int)
    ok_e61: str = field(default_factory=str)
    ok_f61: float = field(default_factory=int)
    ok_g61: bytes = field(default_factory=bytes)
    ok_h61: list[int] = field(default_factory=list)


@dataclass
class Config62:
    bad_a62: float = field(default_factory=str)
    bad_b62: int = field(default_factory=bytes)
    bad_c62: int = field(default_factory=str)
    bad_d62: str = field(default_factory=int)
    ok_a62: set[str] = field(default_factory=set)
    ok_b62: tuple[int, ...] = field(default_factory=tuple)
    ok_c62: int = field(default_factory=int)
    ok_d62: str = field(default_factory=str)
    ok_e62: float = field(default_factory=int)
    ok_f62: bytes = field(default_factory=bytes)
    ok_g62: list[int] = field(default_factory=list)
    ok_h62: dict[str, int] = field(default_factory=dict)


@dataclass
class Config63:
    bad_a63: int = field(default_factory=bytes)
    bad_b63: int = field(default_factory=str)
    bad_c63: str = field(default_factory=int)
    bad_d63: bytes = field(default_factory=float)
    ok_a63: tuple[int, ...] = field(default_factory=tuple)
    ok_b63: int = field(default_factory=int)
    ok_c63: str = field(default_factory=str)
    ok_d63: float = field(default_factory=int)
    ok_e63: bytes = field(default_factory=bytes)
    ok_f63: list[int] = field(default_factory=list)
    ok_g63: dict[str, int] = field(default_factory=dict)
    ok_h63: set[str] = field(default_factory=set)


@dataclass
class Config64:
    bad_a64: int = field(default_factory=str)
    bad_b64: str = field(default_factory=int)
    bad_c64: bytes = field(default_factory=float)
    bad_d64: float | None = field(default_factory=bytes)
    ok_a64: int = field(default_factory=int)
    ok_b64: str = field(default_factory=str)
    ok_c64: float = field(default_factory=int)
    ok_d64: bytes = field(default_factory=bytes)
    ok_e64: list[int] = field(default_factory=list)
    ok_f64: dict[str, int] = field(default_factory=dict)
    ok_g64: set[str] = field(default_factory=set)
    ok_h64: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config65:
    bad_a65: str = field(default_factory=int)
    bad_b65: bytes = field(default_factory=float)
    bad_c65: float | None = field(default_factory=bytes)
    bad_d65: str | None = field(default_factory=bool)
    ok_a65: str = field(default_factory=str)
    ok_b65: float = field(default_factory=int)
    ok_c65: bytes = field(default_factory=bytes)
    ok_d65: list[int] = field(default_factory=list)
    ok_e65: dict[str, int] = field(default_factory=dict)
    ok_f65: set[str] = field(default_factory=set)
    ok_g65: tuple[int, ...] = field(default_factory=tuple)
    ok_h65: int = field(default_factory=int)


@dataclass
class Config66:
    bad_a66: bytes = field(default_factory=float)
    bad_b66: float | None = field(default_factory=bytes)
    bad_c66: str | None = field(default_factory=bool)
    bad_d66: bytes = field(default_factory=int)
    ok_a66: float = field(default_factory=int)
    ok_b66: bytes = field(default_factory=bytes)
    ok_c66: list[int] = field(default_factory=list)
    ok_d66: dict[str, int] = field(default_factory=dict)
    ok_e66: set[str] = field(default_factory=set)
    ok_f66: tuple[int, ...] = field(default_factory=tuple)
    ok_g66: int = field(default_factory=int)
    ok_h66: str = field(default_factory=str)


@dataclass
class Config67:
    bad_a67: float | None = field(default_factory=bytes)
    bad_b67: str | None = field(default_factory=bool)
    bad_c67: bytes = field(default_factory=int)
    bad_d67: float = field(default_factory=str)
    ok_a67: bytes = field(default_factory=bytes)
    ok_b67: list[int] = field(default_factory=list)
    ok_c67: dict[str, int] = field(default_factory=dict)
    ok_d67: set[str] = field(default_factory=set)
    ok_e67: tuple[int, ...] = field(default_factory=tuple)
    ok_f67: int = field(default_factory=int)
    ok_g67: str = field(default_factory=str)
    ok_h67: float = field(default_factory=int)


@dataclass
class Config68:
    bad_a68: str | None = field(default_factory=bool)
    bad_b68: bytes = field(default_factory=int)
    bad_c68: float = field(default_factory=str)
    bad_d68: int = field(default_factory=bytes)
    ok_a68: list[int] = field(default_factory=list)
    ok_b68: dict[str, int] = field(default_factory=dict)
    ok_c68: set[str] = field(default_factory=set)
    ok_d68: tuple[int, ...] = field(default_factory=tuple)
    ok_e68: int = field(default_factory=int)
    ok_f68: str = field(default_factory=str)
    ok_g68: float = field(default_factory=int)
    ok_h68: bytes = field(default_factory=bytes)


@dataclass
class Config69:
    bad_a69: bytes = field(default_factory=int)
    bad_b69: float = field(default_factory=str)
    bad_c69: int = field(default_factory=bytes)
    bad_d69: int = field(default_factory=str)
    ok_a69: dict[str, int] = field(default_factory=dict)
    ok_b69: set[str] = field(default_factory=set)
    ok_c69: tuple[int, ...] = field(default_factory=tuple)
    ok_d69: int = field(default_factory=int)
    ok_e69: str = field(default_factory=str)
    ok_f69: float = field(default_factory=int)
    ok_g69: bytes = field(default_factory=bytes)
    ok_h69: list[int] = field(default_factory=list)


@dataclass
class Config70:
    bad_a70: float = field(default_factory=str)
    bad_b70: int = field(default_factory=bytes)
    bad_c70: int = field(default_factory=str)
    bad_d70: str = field(default_factory=int)
    ok_a70: set[str] = field(default_factory=set)
    ok_b70: tuple[int, ...] = field(default_factory=tuple)
    ok_c70: int = field(default_factory=int)
    ok_d70: str = field(default_factory=str)
    ok_e70: float = field(default_factory=int)
    ok_f70: bytes = field(default_factory=bytes)
    ok_g70: list[int] = field(default_factory=list)
    ok_h70: dict[str, int] = field(default_factory=dict)


@dataclass
class Config71:
    bad_a71: int = field(default_factory=bytes)
    bad_b71: int = field(default_factory=str)
    bad_c71: str = field(default_factory=int)
    bad_d71: bytes = field(default_factory=float)
    ok_a71: tuple[int, ...] = field(default_factory=tuple)
    ok_b71: int = field(default_factory=int)
    ok_c71: str = field(default_factory=str)
    ok_d71: float = field(default_factory=int)
    ok_e71: bytes = field(default_factory=bytes)
    ok_f71: list[int] = field(default_factory=list)
    ok_g71: dict[str, int] = field(default_factory=dict)
    ok_h71: set[str] = field(default_factory=set)


@dataclass
class Config72:
    bad_a72: int = field(default_factory=str)
    bad_b72: str = field(default_factory=int)
    bad_c72: bytes = field(default_factory=float)
    bad_d72: float | None = field(default_factory=bytes)
    ok_a72: int = field(default_factory=int)
    ok_b72: str = field(default_factory=str)
    ok_c72: float = field(default_factory=int)
    ok_d72: bytes = field(default_factory=bytes)
    ok_e72: list[int] = field(default_factory=list)
    ok_f72: dict[str, int] = field(default_factory=dict)
    ok_g72: set[str] = field(default_factory=set)
    ok_h72: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config73:
    bad_a73: str = field(default_factory=int)
    bad_b73: bytes = field(default_factory=float)
    bad_c73: float | None = field(default_factory=bytes)
    bad_d73: str | None = field(default_factory=bool)
    ok_a73: str = field(default_factory=str)
    ok_b73: float = field(default_factory=int)
    ok_c73: bytes = field(default_factory=bytes)
    ok_d73: list[int] = field(default_factory=list)
    ok_e73: dict[str, int] = field(default_factory=dict)
    ok_f73: set[str] = field(default_factory=set)
    ok_g73: tuple[int, ...] = field(default_factory=tuple)
    ok_h73: int = field(default_factory=int)


@dataclass
class Config74:
    bad_a74: bytes = field(default_factory=float)
    bad_b74: float | None = field(default_factory=bytes)
    bad_c74: str | None = field(default_factory=bool)
    bad_d74: bytes = field(default_factory=int)
    ok_a74: float = field(default_factory=int)
    ok_b74: bytes = field(default_factory=bytes)
    ok_c74: list[int] = field(default_factory=list)
    ok_d74: dict[str, int] = field(default_factory=dict)
    ok_e74: set[str] = field(default_factory=set)
    ok_f74: tuple[int, ...] = field(default_factory=tuple)
    ok_g74: int = field(default_factory=int)
    ok_h74: str = field(default_factory=str)


@dataclass
class Config75:
    bad_a75: float | None = field(default_factory=bytes)
    bad_b75: str | None = field(default_factory=bool)
    bad_c75: bytes = field(default_factory=int)
    bad_d75: float = field(default_factory=str)
    ok_a75: bytes = field(default_factory=bytes)
    ok_b75: list[int] = field(default_factory=list)
    ok_c75: dict[str, int] = field(default_factory=dict)
    ok_d75: set[str] = field(default_factory=set)
    ok_e75: tuple[int, ...] = field(default_factory=tuple)
    ok_f75: int = field(default_factory=int)
    ok_g75: str = field(default_factory=str)
    ok_h75: float = field(default_factory=int)


@dataclass
class Config76:
    bad_a76: str | None = field(default_factory=bool)
    bad_b76: bytes = field(default_factory=int)
    bad_c76: float = field(default_factory=str)
    bad_d76: int = field(default_factory=bytes)
    ok_a76: list[int] = field(default_factory=list)
    ok_b76: dict[str, int] = field(default_factory=dict)
    ok_c76: set[str] = field(default_factory=set)
    ok_d76: tuple[int, ...] = field(default_factory=tuple)
    ok_e76: int = field(default_factory=int)
    ok_f76: str = field(default_factory=str)
    ok_g76: float = field(default_factory=int)
    ok_h76: bytes = field(default_factory=bytes)


@dataclass
class Config77:
    bad_a77: bytes = field(default_factory=int)
    bad_b77: float = field(default_factory=str)
    bad_c77: int = field(default_factory=bytes)
    bad_d77: int = field(default_factory=str)
    ok_a77: dict[str, int] = field(default_factory=dict)
    ok_b77: set[str] = field(default_factory=set)
    ok_c77: tuple[int, ...] = field(default_factory=tuple)
    ok_d77: int = field(default_factory=int)
    ok_e77: str = field(default_factory=str)
    ok_f77: float = field(default_factory=int)
    ok_g77: bytes = field(default_factory=bytes)
    ok_h77: list[int] = field(default_factory=list)


@dataclass
class Config78:
    bad_a78: float = field(default_factory=str)
    bad_b78: int = field(default_factory=bytes)
    bad_c78: int = field(default_factory=str)
    bad_d78: str = field(default_factory=int)
    ok_a78: set[str] = field(default_factory=set)
    ok_b78: tuple[int, ...] = field(default_factory=tuple)
    ok_c78: int = field(default_factory=int)
    ok_d78: str = field(default_factory=str)
    ok_e78: float = field(default_factory=int)
    ok_f78: bytes = field(default_factory=bytes)
    ok_g78: list[int] = field(default_factory=list)
    ok_h78: dict[str, int] = field(default_factory=dict)


@dataclass
class Config79:
    bad_a79: int = field(default_factory=bytes)
    bad_b79: int = field(default_factory=str)
    bad_c79: str = field(default_factory=int)
    bad_d79: bytes = field(default_factory=float)
    ok_a79: tuple[int, ...] = field(default_factory=tuple)
    ok_b79: int = field(default_factory=int)
    ok_c79: str = field(default_factory=str)
    ok_d79: float = field(default_factory=int)
    ok_e79: bytes = field(default_factory=bytes)
    ok_f79: list[int] = field(default_factory=list)
    ok_g79: dict[str, int] = field(default_factory=dict)
    ok_h79: set[str] = field(default_factory=set)


@dataclass
class Config80:
    bad_a80: int = field(default_factory=str)
    bad_b80: str = field(default_factory=int)
    bad_c80: bytes = field(default_factory=float)
    bad_d80: float | None = field(default_factory=bytes)
    ok_a80: int = field(default_factory=int)
    ok_b80: str = field(default_factory=str)
    ok_c80: float = field(default_factory=int)
    ok_d80: bytes = field(default_factory=bytes)
    ok_e80: list[int] = field(default_factory=list)
    ok_f80: dict[str, int] = field(default_factory=dict)
    ok_g80: set[str] = field(default_factory=set)
    ok_h80: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config81:
    bad_a81: str = field(default_factory=int)
    bad_b81: bytes = field(default_factory=float)
    bad_c81: float | None = field(default_factory=bytes)
    bad_d81: str | None = field(default_factory=bool)
    ok_a81: str = field(default_factory=str)
    ok_b81: float = field(default_factory=int)
    ok_c81: bytes = field(default_factory=bytes)
    ok_d81: list[int] = field(default_factory=list)
    ok_e81: dict[str, int] = field(default_factory=dict)
    ok_f81: set[str] = field(default_factory=set)
    ok_g81: tuple[int, ...] = field(default_factory=tuple)
    ok_h81: int = field(default_factory=int)


@dataclass
class Config82:
    bad_a82: bytes = field(default_factory=float)
    bad_b82: float | None = field(default_factory=bytes)
    bad_c82: str | None = field(default_factory=bool)
    bad_d82: bytes = field(default_factory=int)
    ok_a82: float = field(default_factory=int)
    ok_b82: bytes = field(default_factory=bytes)
    ok_c82: list[int] = field(default_factory=list)
    ok_d82: dict[str, int] = field(default_factory=dict)
    ok_e82: set[str] = field(default_factory=set)
    ok_f82: tuple[int, ...] = field(default_factory=tuple)
    ok_g82: int = field(default_factory=int)
    ok_h82: str = field(default_factory=str)


@dataclass
class Config83:
    bad_a83: float | None = field(default_factory=bytes)
    bad_b83: str | None = field(default_factory=bool)
    bad_c83: bytes = field(default_factory=int)
    bad_d83: float = field(default_factory=str)
    ok_a83: bytes = field(default_factory=bytes)
    ok_b83: list[int] = field(default_factory=list)
    ok_c83: dict[str, int] = field(default_factory=dict)
    ok_d83: set[str] = field(default_factory=set)
    ok_e83: tuple[int, ...] = field(default_factory=tuple)
    ok_f83: int = field(default_factory=int)
    ok_g83: str = field(default_factory=str)
    ok_h83: float = field(default_factory=int)


@dataclass
class Config84:
    bad_a84: str | None = field(default_factory=bool)
    bad_b84: bytes = field(default_factory=int)
    bad_c84: float = field(default_factory=str)
    bad_d84: int = field(default_factory=bytes)
    ok_a84: list[int] = field(default_factory=list)
    ok_b84: dict[str, int] = field(default_factory=dict)
    ok_c84: set[str] = field(default_factory=set)
    ok_d84: tuple[int, ...] = field(default_factory=tuple)
    ok_e84: int = field(default_factory=int)
    ok_f84: str = field(default_factory=str)
    ok_g84: float = field(default_factory=int)
    ok_h84: bytes = field(default_factory=bytes)


@dataclass
class Config85:
    bad_a85: bytes = field(default_factory=int)
    bad_b85: float = field(default_factory=str)
    bad_c85: int = field(default_factory=bytes)
    bad_d85: int = field(default_factory=str)
    ok_a85: dict[str, int] = field(default_factory=dict)
    ok_b85: set[str] = field(default_factory=set)
    ok_c85: tuple[int, ...] = field(default_factory=tuple)
    ok_d85: int = field(default_factory=int)
    ok_e85: str = field(default_factory=str)
    ok_f85: float = field(default_factory=int)
    ok_g85: bytes = field(default_factory=bytes)
    ok_h85: list[int] = field(default_factory=list)


@dataclass
class Config86:
    bad_a86: float = field(default_factory=str)
    bad_b86: int = field(default_factory=bytes)
    bad_c86: int = field(default_factory=str)
    bad_d86: str = field(default_factory=int)
    ok_a86: set[str] = field(default_factory=set)
    ok_b86: tuple[int, ...] = field(default_factory=tuple)
    ok_c86: int = field(default_factory=int)
    ok_d86: str = field(default_factory=str)
    ok_e86: float = field(default_factory=int)
    ok_f86: bytes = field(default_factory=bytes)
    ok_g86: list[int] = field(default_factory=list)
    ok_h86: dict[str, int] = field(default_factory=dict)


@dataclass
class Config87:
    bad_a87: int = field(default_factory=bytes)
    bad_b87: int = field(default_factory=str)
    bad_c87: str = field(default_factory=int)
    bad_d87: bytes = field(default_factory=float)
    ok_a87: tuple[int, ...] = field(default_factory=tuple)
    ok_b87: int = field(default_factory=int)
    ok_c87: str = field(default_factory=str)
    ok_d87: float = field(default_factory=int)
    ok_e87: bytes = field(default_factory=bytes)
    ok_f87: list[int] = field(default_factory=list)
    ok_g87: dict[str, int] = field(default_factory=dict)
    ok_h87: set[str] = field(default_factory=set)


@dataclass
class Config88:
    bad_a88: int = field(default_factory=str)
    bad_b88: str = field(default_factory=int)
    bad_c88: bytes = field(default_factory=float)
    bad_d88: float | None = field(default_factory=bytes)
    ok_a88: int = field(default_factory=int)
    ok_b88: str = field(default_factory=str)
    ok_c88: float = field(default_factory=int)
    ok_d88: bytes = field(default_factory=bytes)
    ok_e88: list[int] = field(default_factory=list)
    ok_f88: dict[str, int] = field(default_factory=dict)
    ok_g88: set[str] = field(default_factory=set)
    ok_h88: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config89:
    bad_a89: str = field(default_factory=int)
    bad_b89: bytes = field(default_factory=float)
    bad_c89: float | None = field(default_factory=bytes)
    bad_d89: str | None = field(default_factory=bool)
    ok_a89: str = field(default_factory=str)
    ok_b89: float = field(default_factory=int)
    ok_c89: bytes = field(default_factory=bytes)
    ok_d89: list[int] = field(default_factory=list)
    ok_e89: dict[str, int] = field(default_factory=dict)
    ok_f89: set[str] = field(default_factory=set)
    ok_g89: tuple[int, ...] = field(default_factory=tuple)
    ok_h89: int = field(default_factory=int)


@dataclass
class Config90:
    bad_a90: bytes = field(default_factory=float)
    bad_b90: float | None = field(default_factory=bytes)
    bad_c90: str | None = field(default_factory=bool)
    bad_d90: bytes = field(default_factory=int)
    ok_a90: float = field(default_factory=int)
    ok_b90: bytes = field(default_factory=bytes)
    ok_c90: list[int] = field(default_factory=list)
    ok_d90: dict[str, int] = field(default_factory=dict)
    ok_e90: set[str] = field(default_factory=set)
    ok_f90: tuple[int, ...] = field(default_factory=tuple)
    ok_g90: int = field(default_factory=int)
    ok_h90: str = field(default_factory=str)


@dataclass
class Config91:
    bad_a91: float | None = field(default_factory=bytes)
    bad_b91: str | None = field(default_factory=bool)
    bad_c91: bytes = field(default_factory=int)
    bad_d91: float = field(default_factory=str)
    ok_a91: bytes = field(default_factory=bytes)
    ok_b91: list[int] = field(default_factory=list)
    ok_c91: dict[str, int] = field(default_factory=dict)
    ok_d91: set[str] = field(default_factory=set)
    ok_e91: tuple[int, ...] = field(default_factory=tuple)
    ok_f91: int = field(default_factory=int)
    ok_g91: str = field(default_factory=str)
    ok_h91: float = field(default_factory=int)


@dataclass
class Config92:
    bad_a92: str | None = field(default_factory=bool)
    bad_b92: bytes = field(default_factory=int)
    bad_c92: float = field(default_factory=str)
    bad_d92: int = field(default_factory=bytes)
    ok_a92: list[int] = field(default_factory=list)
    ok_b92: dict[str, int] = field(default_factory=dict)
    ok_c92: set[str] = field(default_factory=set)
    ok_d92: tuple[int, ...] = field(default_factory=tuple)
    ok_e92: int = field(default_factory=int)
    ok_f92: str = field(default_factory=str)
    ok_g92: float = field(default_factory=int)
    ok_h92: bytes = field(default_factory=bytes)


@dataclass
class Config93:
    bad_a93: bytes = field(default_factory=int)
    bad_b93: float = field(default_factory=str)
    bad_c93: int = field(default_factory=bytes)
    bad_d93: int = field(default_factory=str)
    ok_a93: dict[str, int] = field(default_factory=dict)
    ok_b93: set[str] = field(default_factory=set)
    ok_c93: tuple[int, ...] = field(default_factory=tuple)
    ok_d93: int = field(default_factory=int)
    ok_e93: str = field(default_factory=str)
    ok_f93: float = field(default_factory=int)
    ok_g93: bytes = field(default_factory=bytes)
    ok_h93: list[int] = field(default_factory=list)


@dataclass
class Config94:
    bad_a94: float = field(default_factory=str)
    bad_b94: int = field(default_factory=bytes)
    bad_c94: int = field(default_factory=str)
    bad_d94: str = field(default_factory=int)
    ok_a94: set[str] = field(default_factory=set)
    ok_b94: tuple[int, ...] = field(default_factory=tuple)
    ok_c94: int = field(default_factory=int)
    ok_d94: str = field(default_factory=str)
    ok_e94: float = field(default_factory=int)
    ok_f94: bytes = field(default_factory=bytes)
    ok_g94: list[int] = field(default_factory=list)
    ok_h94: dict[str, int] = field(default_factory=dict)


@dataclass
class Config95:
    bad_a95: int = field(default_factory=bytes)
    bad_b95: int = field(default_factory=str)
    bad_c95: str = field(default_factory=int)
    bad_d95: bytes = field(default_factory=float)
    ok_a95: tuple[int, ...] = field(default_factory=tuple)
    ok_b95: int = field(default_factory=int)
    ok_c95: str = field(default_factory=str)
    ok_d95: float = field(default_factory=int)
    ok_e95: bytes = field(default_factory=bytes)
    ok_f95: list[int] = field(default_factory=list)
    ok_g95: dict[str, int] = field(default_factory=dict)
    ok_h95: set[str] = field(default_factory=set)


@dataclass
class Config96:
    bad_a96: int = field(default_factory=str)
    bad_b96: str = field(default_factory=int)
    bad_c96: bytes = field(default_factory=float)
    bad_d96: float | None = field(default_factory=bytes)
    ok_a96: int = field(default_factory=int)
    ok_b96: str = field(default_factory=str)
    ok_c96: float = field(default_factory=int)
    ok_d96: bytes = field(default_factory=bytes)
    ok_e96: list[int] = field(default_factory=list)
    ok_f96: dict[str, int] = field(default_factory=dict)
    ok_g96: set[str] = field(default_factory=set)
    ok_h96: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config97:
    bad_a97: str = field(default_factory=int)
    bad_b97: bytes = field(default_factory=float)
    bad_c97: float | None = field(default_factory=bytes)
    bad_d97: str | None = field(default_factory=bool)
    ok_a97: str = field(default_factory=str)
    ok_b97: float = field(default_factory=int)
    ok_c97: bytes = field(default_factory=bytes)
    ok_d97: list[int] = field(default_factory=list)
    ok_e97: dict[str, int] = field(default_factory=dict)
    ok_f97: set[str] = field(default_factory=set)
    ok_g97: tuple[int, ...] = field(default_factory=tuple)
    ok_h97: int = field(default_factory=int)


@dataclass
class Config98:
    bad_a98: bytes = field(default_factory=float)
    bad_b98: float | None = field(default_factory=bytes)
    bad_c98: str | None = field(default_factory=bool)
    bad_d98: bytes = field(default_factory=int)
    ok_a98: float = field(default_factory=int)
    ok_b98: bytes = field(default_factory=bytes)
    ok_c98: list[int] = field(default_factory=list)
    ok_d98: dict[str, int] = field(default_factory=dict)
    ok_e98: set[str] = field(default_factory=set)
    ok_f98: tuple[int, ...] = field(default_factory=tuple)
    ok_g98: int = field(default_factory=int)
    ok_h98: str = field(default_factory=str)


@dataclass
class Config99:
    bad_a99: float | None = field(default_factory=bytes)
    bad_b99: str | None = field(default_factory=bool)
    bad_c99: bytes = field(default_factory=int)
    bad_d99: float = field(default_factory=str)
    ok_a99: bytes = field(default_factory=bytes)
    ok_b99: list[int] = field(default_factory=list)
    ok_c99: dict[str, int] = field(default_factory=dict)
    ok_d99: set[str] = field(default_factory=set)
    ok_e99: tuple[int, ...] = field(default_factory=tuple)
    ok_f99: int = field(default_factory=int)
    ok_g99: str = field(default_factory=str)
    ok_h99: float = field(default_factory=int)


@dataclass
class Config100:
    bad_a100: str | None = field(default_factory=bool)
    bad_b100: bytes = field(default_factory=int)
    bad_c100: float = field(default_factory=str)
    bad_d100: int = field(default_factory=bytes)
    ok_a100: list[int] = field(default_factory=list)
    ok_b100: dict[str, int] = field(default_factory=dict)
    ok_c100: set[str] = field(default_factory=set)
    ok_d100: tuple[int, ...] = field(default_factory=tuple)
    ok_e100: int = field(default_factory=int)
    ok_f100: str = field(default_factory=str)
    ok_g100: float = field(default_factory=int)
    ok_h100: bytes = field(default_factory=bytes)


@dataclass
class Config101:
    bad_a101: bytes = field(default_factory=int)
    bad_b101: float = field(default_factory=str)
    bad_c101: int = field(default_factory=bytes)
    bad_d101: int = field(default_factory=str)
    ok_a101: dict[str, int] = field(default_factory=dict)
    ok_b101: set[str] = field(default_factory=set)
    ok_c101: tuple[int, ...] = field(default_factory=tuple)
    ok_d101: int = field(default_factory=int)
    ok_e101: str = field(default_factory=str)
    ok_f101: float = field(default_factory=int)
    ok_g101: bytes = field(default_factory=bytes)
    ok_h101: list[int] = field(default_factory=list)


@dataclass
class Config102:
    bad_a102: float = field(default_factory=str)
    bad_b102: int = field(default_factory=bytes)
    bad_c102: int = field(default_factory=str)
    bad_d102: str = field(default_factory=int)
    ok_a102: set[str] = field(default_factory=set)
    ok_b102: tuple[int, ...] = field(default_factory=tuple)
    ok_c102: int = field(default_factory=int)
    ok_d102: str = field(default_factory=str)
    ok_e102: float = field(default_factory=int)
    ok_f102: bytes = field(default_factory=bytes)
    ok_g102: list[int] = field(default_factory=list)
    ok_h102: dict[str, int] = field(default_factory=dict)


@dataclass
class Config103:
    bad_a103: int = field(default_factory=bytes)
    bad_b103: int = field(default_factory=str)
    bad_c103: str = field(default_factory=int)
    bad_d103: bytes = field(default_factory=float)
    ok_a103: tuple[int, ...] = field(default_factory=tuple)
    ok_b103: int = field(default_factory=int)
    ok_c103: str = field(default_factory=str)
    ok_d103: float = field(default_factory=int)
    ok_e103: bytes = field(default_factory=bytes)
    ok_f103: list[int] = field(default_factory=list)
    ok_g103: dict[str, int] = field(default_factory=dict)
    ok_h103: set[str] = field(default_factory=set)


@dataclass
class Config104:
    bad_a104: int = field(default_factory=str)
    bad_b104: str = field(default_factory=int)
    bad_c104: bytes = field(default_factory=float)
    bad_d104: float | None = field(default_factory=bytes)
    ok_a104: int = field(default_factory=int)
    ok_b104: str = field(default_factory=str)
    ok_c104: float = field(default_factory=int)
    ok_d104: bytes = field(default_factory=bytes)
    ok_e104: list[int] = field(default_factory=list)
    ok_f104: dict[str, int] = field(default_factory=dict)
    ok_g104: set[str] = field(default_factory=set)
    ok_h104: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config105:
    bad_a105: str = field(default_factory=int)
    bad_b105: bytes = field(default_factory=float)
    bad_c105: float | None = field(default_factory=bytes)
    bad_d105: str | None = field(default_factory=bool)
    ok_a105: str = field(default_factory=str)
    ok_b105: float = field(default_factory=int)
    ok_c105: bytes = field(default_factory=bytes)
    ok_d105: list[int] = field(default_factory=list)
    ok_e105: dict[str, int] = field(default_factory=dict)
    ok_f105: set[str] = field(default_factory=set)
    ok_g105: tuple[int, ...] = field(default_factory=tuple)
    ok_h105: int = field(default_factory=int)


@dataclass
class Config106:
    bad_a106: bytes = field(default_factory=float)
    bad_b106: float | None = field(default_factory=bytes)
    bad_c106: str | None = field(default_factory=bool)
    bad_d106: bytes = field(default_factory=int)
    ok_a106: float = field(default_factory=int)
    ok_b106: bytes = field(default_factory=bytes)
    ok_c106: list[int] = field(default_factory=list)
    ok_d106: dict[str, int] = field(default_factory=dict)
    ok_e106: set[str] = field(default_factory=set)
    ok_f106: tuple[int, ...] = field(default_factory=tuple)
    ok_g106: int = field(default_factory=int)
    ok_h106: str = field(default_factory=str)


@dataclass
class Config107:
    bad_a107: float | None = field(default_factory=bytes)
    bad_b107: str | None = field(default_factory=bool)
    bad_c107: bytes = field(default_factory=int)
    bad_d107: float = field(default_factory=str)
    ok_a107: bytes = field(default_factory=bytes)
    ok_b107: list[int] = field(default_factory=list)
    ok_c107: dict[str, int] = field(default_factory=dict)
    ok_d107: set[str] = field(default_factory=set)
    ok_e107: tuple[int, ...] = field(default_factory=tuple)
    ok_f107: int = field(default_factory=int)
    ok_g107: str = field(default_factory=str)
    ok_h107: float = field(default_factory=int)


@dataclass
class Config108:
    bad_a108: str | None = field(default_factory=bool)
    bad_b108: bytes = field(default_factory=int)
    bad_c108: float = field(default_factory=str)
    bad_d108: int = field(default_factory=bytes)
    ok_a108: list[int] = field(default_factory=list)
    ok_b108: dict[str, int] = field(default_factory=dict)
    ok_c108: set[str] = field(default_factory=set)
    ok_d108: tuple[int, ...] = field(default_factory=tuple)
    ok_e108: int = field(default_factory=int)
    ok_f108: str = field(default_factory=str)
    ok_g108: float = field(default_factory=int)
    ok_h108: bytes = field(default_factory=bytes)


@dataclass
class Config109:
    bad_a109: bytes = field(default_factory=int)
    bad_b109: float = field(default_factory=str)
    bad_c109: int = field(default_factory=bytes)
    bad_d109: int = field(default_factory=str)
    ok_a109: dict[str, int] = field(default_factory=dict)
    ok_b109: set[str] = field(default_factory=set)
    ok_c109: tuple[int, ...] = field(default_factory=tuple)
    ok_d109: int = field(default_factory=int)
    ok_e109: str = field(default_factory=str)
    ok_f109: float = field(default_factory=int)
    ok_g109: bytes = field(default_factory=bytes)
    ok_h109: list[int] = field(default_factory=list)


@dataclass
class Config110:
    bad_a110: float = field(default_factory=str)
    bad_b110: int = field(default_factory=bytes)
    bad_c110: int = field(default_factory=str)
    bad_d110: str = field(default_factory=int)
    ok_a110: set[str] = field(default_factory=set)
    ok_b110: tuple[int, ...] = field(default_factory=tuple)
    ok_c110: int = field(default_factory=int)
    ok_d110: str = field(default_factory=str)
    ok_e110: float = field(default_factory=int)
    ok_f110: bytes = field(default_factory=bytes)
    ok_g110: list[int] = field(default_factory=list)
    ok_h110: dict[str, int] = field(default_factory=dict)


@dataclass
class Config111:
    bad_a111: int = field(default_factory=bytes)
    bad_b111: int = field(default_factory=str)
    bad_c111: str = field(default_factory=int)
    bad_d111: bytes = field(default_factory=float)
    ok_a111: tuple[int, ...] = field(default_factory=tuple)
    ok_b111: int = field(default_factory=int)
    ok_c111: str = field(default_factory=str)
    ok_d111: float = field(default_factory=int)
    ok_e111: bytes = field(default_factory=bytes)
    ok_f111: list[int] = field(default_factory=list)
    ok_g111: dict[str, int] = field(default_factory=dict)
    ok_h111: set[str] = field(default_factory=set)


@dataclass
class Config112:
    bad_a112: int = field(default_factory=str)
    bad_b112: str = field(default_factory=int)
    bad_c112: bytes = field(default_factory=float)
    bad_d112: float | None = field(default_factory=bytes)
    ok_a112: int = field(default_factory=int)
    ok_b112: str = field(default_factory=str)
    ok_c112: float = field(default_factory=int)
    ok_d112: bytes = field(default_factory=bytes)
    ok_e112: list[int] = field(default_factory=list)
    ok_f112: dict[str, int] = field(default_factory=dict)
    ok_g112: set[str] = field(default_factory=set)
    ok_h112: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config113:
    bad_a113: str = field(default_factory=int)
    bad_b113: bytes = field(default_factory=float)
    bad_c113: float | None = field(default_factory=bytes)
    bad_d113: str | None = field(default_factory=bool)
    ok_a113: str = field(default_factory=str)
    ok_b113: float = field(default_factory=int)
    ok_c113: bytes = field(default_factory=bytes)
    ok_d113: list[int] = field(default_factory=list)
    ok_e113: dict[str, int] = field(default_factory=dict)
    ok_f113: set[str] = field(default_factory=set)
    ok_g113: tuple[int, ...] = field(default_factory=tuple)
    ok_h113: int = field(default_factory=int)


@dataclass
class Config114:
    bad_a114: bytes = field(default_factory=float)
    bad_b114: float | None = field(default_factory=bytes)
    bad_c114: str | None = field(default_factory=bool)
    bad_d114: bytes = field(default_factory=int)
    ok_a114: float = field(default_factory=int)
    ok_b114: bytes = field(default_factory=bytes)
    ok_c114: list[int] = field(default_factory=list)
    ok_d114: dict[str, int] = field(default_factory=dict)
    ok_e114: set[str] = field(default_factory=set)
    ok_f114: tuple[int, ...] = field(default_factory=tuple)
    ok_g114: int = field(default_factory=int)
    ok_h114: str = field(default_factory=str)


@dataclass
class Config115:
    bad_a115: float | None = field(default_factory=bytes)
    bad_b115: str | None = field(default_factory=bool)
    bad_c115: bytes = field(default_factory=int)
    bad_d115: float = field(default_factory=str)
    ok_a115: bytes = field(default_factory=bytes)
    ok_b115: list[int] = field(default_factory=list)
    ok_c115: dict[str, int] = field(default_factory=dict)
    ok_d115: set[str] = field(default_factory=set)
    ok_e115: tuple[int, ...] = field(default_factory=tuple)
    ok_f115: int = field(default_factory=int)
    ok_g115: str = field(default_factory=str)
    ok_h115: float = field(default_factory=int)


@dataclass
class Config116:
    bad_a116: str | None = field(default_factory=bool)
    bad_b116: bytes = field(default_factory=int)
    bad_c116: float = field(default_factory=str)
    bad_d116: int = field(default_factory=bytes)
    ok_a116: list[int] = field(default_factory=list)
    ok_b116: dict[str, int] = field(default_factory=dict)
    ok_c116: set[str] = field(default_factory=set)
    ok_d116: tuple[int, ...] = field(default_factory=tuple)
    ok_e116: int = field(default_factory=int)
    ok_f116: str = field(default_factory=str)
    ok_g116: float = field(default_factory=int)
    ok_h116: bytes = field(default_factory=bytes)


@dataclass
class Config117:
    bad_a117: bytes = field(default_factory=int)
    bad_b117: float = field(default_factory=str)
    bad_c117: int = field(default_factory=bytes)
    bad_d117: int = field(default_factory=str)
    ok_a117: dict[str, int] = field(default_factory=dict)
    ok_b117: set[str] = field(default_factory=set)
    ok_c117: tuple[int, ...] = field(default_factory=tuple)
    ok_d117: int = field(default_factory=int)
    ok_e117: str = field(default_factory=str)
    ok_f117: float = field(default_factory=int)
    ok_g117: bytes = field(default_factory=bytes)
    ok_h117: list[int] = field(default_factory=list)


@dataclass
class Config118:
    bad_a118: float = field(default_factory=str)
    bad_b118: int = field(default_factory=bytes)
    bad_c118: int = field(default_factory=str)
    bad_d118: str = field(default_factory=int)
    ok_a118: set[str] = field(default_factory=set)
    ok_b118: tuple[int, ...] = field(default_factory=tuple)
    ok_c118: int = field(default_factory=int)
    ok_d118: str = field(default_factory=str)
    ok_e118: float = field(default_factory=int)
    ok_f118: bytes = field(default_factory=bytes)
    ok_g118: list[int] = field(default_factory=list)
    ok_h118: dict[str, int] = field(default_factory=dict)


@dataclass
class Config119:
    bad_a119: int = field(default_factory=bytes)
    bad_b119: int = field(default_factory=str)
    bad_c119: str = field(default_factory=int)
    bad_d119: bytes = field(default_factory=float)
    ok_a119: tuple[int, ...] = field(default_factory=tuple)
    ok_b119: int = field(default_factory=int)
    ok_c119: str = field(default_factory=str)
    ok_d119: float = field(default_factory=int)
    ok_e119: bytes = field(default_factory=bytes)
    ok_f119: list[int] = field(default_factory=list)
    ok_g119: dict[str, int] = field(default_factory=dict)
    ok_h119: set[str] = field(default_factory=set)


@dataclass
class Config120:
    bad_a120: int = field(default_factory=str)
    bad_b120: str = field(default_factory=int)
    bad_c120: bytes = field(default_factory=float)
    bad_d120: float | None = field(default_factory=bytes)
    ok_a120: int = field(default_factory=int)
    ok_b120: str = field(default_factory=str)
    ok_c120: float = field(default_factory=int)
    ok_d120: bytes = field(default_factory=bytes)
    ok_e120: list[int] = field(default_factory=list)
    ok_f120: dict[str, int] = field(default_factory=dict)
    ok_g120: set[str] = field(default_factory=set)
    ok_h120: tuple[int, ...] = field(default_factory=tuple)


@dataclass
class Config121:
    bad_a121: str = field(default_factory=int)
    bad_b121: bytes = field(default_factory=float)
    bad_c121: float | None = field(default_factory=bytes)
    bad_d121: str | None = field(default_factory=bool)
    ok_a121: str = field(default_factory=str)
    ok_b121: float = field(default_factory=int)
    ok_c121: bytes = field(default_factory=bytes)
    ok_d121: list[int] = field(default_factory=list)
    ok_e121: dict[str, int] = field(default_factory=dict)
    ok_f121: set[str] = field(default_factory=set)
    ok_g121: tuple[int, ...] = field(default_factory=tuple)
    ok_h121: int = field(default_factory=int)


@dataclass
class Config122:
    bad_a122: bytes = field(default_factory=float)
    bad_b122: float | None = field(default_factory=bytes)
    bad_c122: str | None = field(default_factory=bool)
    bad_d122: bytes = field(default_factory=int)
    ok_a122: float = field(default_factory=int)
    ok_b122: bytes = field(default_factory=bytes)
    ok_c122: list[int] = field(default_factory=list)
    ok_d122: dict[str, int] = field(default_factory=dict)
    ok_e122: set[str] = field(default_factory=set)
    ok_f122: tuple[int, ...] = field(default_factory=tuple)
    ok_g122: int = field(default_factory=int)
    ok_h122: str = field(default_factory=str)


@dataclass
class Config123:
    bad_a123: float | None = field(default_factory=bytes)
    bad_b123: str | None = field(default_factory=bool)
    bad_c123: bytes = field(default_factory=int)
    bad_d123: float = field(default_factory=str)
    ok_a123: bytes = field(default_factory=bytes)
    ok_b123: list[int] = field(default_factory=list)
    ok_c123: dict[str, int] = field(default_factory=dict)
    ok_d123: set[str] = field(default_factory=set)
    ok_e123: tuple[int, ...] = field(default_factory=tuple)
    ok_f123: int = field(default_factory=int)
    ok_g123: str = field(default_factory=str)
    ok_h123: float = field(default_factory=int)


@dataclass
class Config124:
    bad_a124: str | None = field(default_factory=bool)
    bad_b124: bytes = field(default_factory=int)
    bad_c124: float = field(default_factory=str)
    bad_d124: int = field(default_factory=bytes)
    ok_a124: list[int] = field(default_factory=list)
    ok_b124: dict[str, int] = field(default_factory=dict)
    ok_c124: set[str] = field(default_factory=set)
    ok_d124: tuple[int, ...] = field(default_factory=tuple)
    ok_e124: int = field(default_factory=int)
    ok_f124: str = field(default_factory=str)
    ok_g124: float = field(default_factory=int)
    ok_h124: bytes = field(default_factory=bytes)

