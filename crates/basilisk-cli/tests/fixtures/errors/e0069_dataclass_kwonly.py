from dataclasses import dataclass, KW_ONLY


@dataclass
class Point:
    x: float
    _: KW_ONLY
    y: float = 0.0


Point(1.0)  # OK — x positional, y uses default
Point(1.0, 2.0)  # E0069 — y is keyword-only, cannot be passed positionally
