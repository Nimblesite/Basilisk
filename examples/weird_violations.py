"""
Weird and subtle violations — the cases that fool other type checkers.

These are not contrived: every pattern appears in real codebases.
Basilisk catches all of them.

Run:  cargo run -- check examples/weird_violations.py
"""

from __future__ import annotations

from typing import Any, overload


# ── E0003: empty dict hiding inside a function default ───────────────────────
# (Basilisk checks module-level assignments)
_cache = {}                              # BSK-E0003: type of values unknown


# ── E0014: bool is a subtype of int in Python, but Basilisk still flags
#    assigning a bool literal to a float field ────────────────────────────────
ratio: float = True                      # BSK-E0014: bool literal, not float


# ── E0014: negative int literal assigned to a str field ──────────────────────
sentinel: str = -1                       # BSK-E0014: int, not str


# ── E0017: attribute type flipped from mutable to immutable in child ─────────
class Config:
    values: list[str]
    max_size: int


class FrozenConfig(Config):
    values: tuple[str, ...]  = ()        # BSK-E0017: tuple overrides list
    max_size: str = "unlimited"          # BSK-E0017: str overrides int


# ── E0018: name used before assignment even though it looks like a constant ───
def describe_algorithm() -> str:
    return f"Using {ALGO_NAME} with seed {ALGO_SEED}"  # BSK-E0018: ALGO_NAME not yet defined

ALGO_NAME: str = "DBSCAN"
ALGO_SEED: int = 42


# ── E0019: exactly-one-path binding — the 'elif' still leaves a gap ──────────
def pick_strategy(score: float, mode: str) -> str:
    if score > 0.9:
        strategy = "aggressive"
    elif mode == "safe":
        strategy = "conservative"
    # no else — if score <= 0.9 and mode != "safe", strategy is unbound
    return strategy                      # BSK-E0019: strategy may be unbound


# ── E0019: augmented assignment in a try block ───────────────────────────────
def sum_with_retry(values: list[int], retries: int) -> int:
    if retries > 0:
        result = 0
        for v in values:
            result += v
    return result                        # BSK-E0019: result may be unbound


# ── E0021: unannotated overload params look identical to the checker ─────────
# (differs only in return type — unannotated param means both have same signature)
@overload
def load(path) -> bytes: ...            # BSK-E0001: path untyped


@overload
def load(path) -> str: ...              # BSK-E0001 + BSK-E0021: duplicate


def load(path: str) -> bytes | str:
    with open(path, "rb") as fh:
        return fh.read()


# ── E0021: unannotated + Any together — Any is explicit, param is bare ────────
@overload
def wrap(value) -> list[Any]: ...       # BSK-E0001: value untyped


@overload
def wrap(value) -> list[Any]: ...       # BSK-E0001 + BSK-E0021: duplicate


def wrap(value: Any) -> list[Any]:      # BSK-E0011: Any without justification
    return [value]


# ── E0022: list literal as dict key in a local dict ─────────────────────────
def make_tag_index() -> dict[list[str], float]:
    return {["tag_a", "tag_b"]: 1.0}    # BSK-E0022: list literal as key


# ── E0023: match on an int with only two arms (0 and 1) ─────────────────────
def bool_from_db(raw: int) -> str:
    match raw:
        case 0:
            return "false"
        case 1:
            return "true"
    # BSK-E0023: 2, -1, 99 etc. fall through silently


# ── E0025: override buried inside a mixin chain ──────────────────────────────
class Serializable:
    def to_json(self) -> str:
        return "{}"


class Timestamped:
    def to_json(self) -> str:           # BSK-E0025: no @override (inherits from Serializable via MRO)
        return '{"ts": 0}'


# ── Combination: untyped + Any return + unhashable key ───────────────────────
def batch_lookup(keys, db):             # BSK-E0001: keys, db untyped
    results = {}
    for key in keys:
        results[key] = db.get(key)
    return results                       # BSK-E0002: no return type
