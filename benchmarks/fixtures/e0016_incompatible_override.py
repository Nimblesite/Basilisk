# BSK-E0016: Incompatible method override
# Classes with overridden methods that have incompatible signatures.


class Base:
    def process(self, value: int) -> str: return str(value)
    def compute(self, x: float, y: float) -> float: return x + y
    def transform(self, data: bytes) -> bytes: return data
    def validate(self, item: str) -> bool: return bool(item)
    def convert(self, source: int) -> float: return float(source)
    def encode(self, text: str) -> bytes: return text.encode()
    def decode(self, raw: bytes) -> str: return raw.decode()
    def merge(self, left: int, right: int) -> int: return left + right
    def split(self, value: str, sep: str) -> list[str]: return value.split(sep)
    def clamp(self, value: int, lo: int, hi: int) -> int: return max(lo, min(hi, value))


class Child01(Base):
    def process(self, value: str) -> str: return value  # wrong param type

class Child02(Base):
    def compute(self, x: int, y: int) -> float: return float(x + y)  # param types narrowed

class Child03(Base):
    def transform(self, data: str) -> bytes: return data.encode()  # wrong param type

class Child04(Base):
    def validate(self, item: int) -> bool: return bool(item)  # wrong param type

class Child05(Base):
    def convert(self, source: str) -> float: return float(source)  # wrong param type

class Child06(Base):
    def encode(self, text: bytes) -> bytes: return text  # wrong param type

class Child07(Base):
    def decode(self, raw: str) -> str: return raw  # wrong param type

class Child08(Base):
    def merge(self, left: str, right: str) -> int: return 0  # wrong param types

class Child09(Base):
    def split(self, value: int, sep: str) -> list[str]: return []  # wrong param type

class Child10(Base):
    def clamp(self, value: float, lo: int, hi: int) -> int: return 0  # wrong param type

class Child11(Base):
    def process(self, value: str) -> str: return value

class Child12(Base):
    def compute(self, x: int, y: int) -> float: return float(x)

class Child13(Base):
    def transform(self, data: str) -> bytes: return data.encode()

class Child14(Base):
    def validate(self, item: int) -> bool: return bool(item)

class Child15(Base):
    def convert(self, source: str) -> float: return float(source)

class Child16(Base):
    def encode(self, text: bytes) -> bytes: return text

class Child17(Base):
    def decode(self, raw: str) -> str: return raw

class Child18(Base):
    def merge(self, left: str, right: str) -> int: return 0

class Child19(Base):
    def split(self, value: int, sep: str) -> list[str]: return []

class Child20(Base):
    def clamp(self, value: float, lo: int, hi: int) -> int: return 0
