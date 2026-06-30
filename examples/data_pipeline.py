"""
ETL data pipeline — realistic data engineering code with type violations.

Run:  cargo run -- check examples/data_pipeline.py
"""

from __future__ import annotations

from typing import Any, overload


# ── BSK-E0003: unannotated empty collections at module scope ─────────────────
_schema_cache = {}  # BSK-E0003: empty dict
_transform_registry = []  # BSK-E0003: empty list


# ── BSK-E0001/E0002: untyped ETL stage functions ─────────────────────────────
def extract(source, options):  # BSK-E0001: source, options untyped
    records = source.read_all()
    return records  # BSK-E0002: no return type


def transform(records, schema, strict):  # BSK-E0001: three untyped params
    result = []
    for row in records:
        result.append(row)
    return result  # BSK-E0002: no return type


def load(records, destination):  # BSK-E0001: records, destination untyped
    destination.write(records)
    # implicit return None — no annotation # BSK-E0002: no return type


# ── returns_compatibility: Any annotation without justification ──────────────────────────
def coerce_field(value: Any, target_type: Any) -> Any:  # returns_compatibility ×3
    return target_type(value)


# ── assignment_compatibility: type-incompatible constant assignments ────────────────────────
BATCH_SIZE: int = "1000"  # assignment_compatibility: str, not int
NULL_SENTINEL: float = "NaN"  # assignment_compatibility: str, not float
MAX_ERRORS: int = 0.5  # assignment_compatibility: float, not int


# ── classes_override_2: subclass narrows column type incompatibly ─────────────────────
class Column:
    name: str
    dtype: str
    nullable: bool


class PartitionKey(Column):
    nullable: int = 0  # classes_override_2: int overrides bool


# ── names_undefined: name used before any assignment in the module ─────────────────
def validate_schema(name: str) -> bool:
    return name in _known_types  # names_undefined: _known_types undefined


_known_types: set[str] = {"int", "str", "float", "bool"}


# ── names_unbound: conditionally assigned variable returned unconditionally ───────
def detect_encoding(raw_bytes: bytes) -> str:
    if raw_bytes[:3] == b"\xef\xbb\xbf":
        encoding = "utf-8-sig"
    elif raw_bytes[:2] in (b"\xff\xfe", b"\xfe\xff"):
        encoding = "utf-16"
    # no else branch — encoding may be unbound if no BOM matches
    return encoding  # names_unbound: encoding may be unbound


# ── overloads_consistency: unannotated overload params produce a duplicate ───────────────
@overload
def read_source(path) -> list[dict[str, str]]: ...  # BSK-E0001: path untyped


@overload
def read_source(
    path,
) -> list[dict[str, str]]: ...  # BSK-E0001 + overloads_consistency: duplicate


def read_source(path: str) -> list[dict[str, str]]:
    return []


# ── dict_key_hashable: list literal used as a dict key ───────────────────────────────
def empty_schema() -> dict[list[str], str]:  # unhashable key type in annotation
    return {["a", "b"]: "string"}  # dict_key_hashable: list literal as key


# ── BSK-E0025: override not decorated ───────────────────────────────────────
class BaseWriter:
    def flush(self, data: list[bytes]) -> int:
        return len(data)


class ParquetWriter(BaseWriter):
    def flush(self, data: list[bytes]) -> int:  # BSK-E0025: missing @override
        return len(data) * 2
