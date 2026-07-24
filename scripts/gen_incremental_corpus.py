#!/usr/bin/env python3
"""Generate a synthetic 1M+ LOC Python workspace for the Stage 1 incremental
measurements ([TYPEINF-TARGET-INCREMENTAL], NARROWPLAN-CHECKLIST Stage 1:
"Measure memory on a 1M+ LOC target" / "Measure p50/p99 keystroke re-check
latency on a 1M-LOC corpus").

The corpus is deterministic (seeded) so measurements are reproducible: modules
with annotated functions, classes, module-level constants, container literals,
comprehensions, and cross-module imports — the shapes the definition-level
Salsa queries (crates/basilisk-checker/src/incremental_defs.rs) exercise.

Usage:
    python3 scripts/gen_incremental_corpus.py OUT_DIR [--files N] [--loc-per-file N]

Defaults produce ~1.05M LOC (2100 files x ~500 LOC).
"""

from __future__ import annotations

import argparse
import random
from pathlib import Path

MODULE_TEMPLATE_HEADER = '"""Synthetic module {index} for incremental measurement."""\n'


def gen_function(rng: random.Random, index: int) -> list[str]:
    """One annotated function with a small typed body (~14 lines)."""
    a, b = rng.randint(1, 99), rng.randint(1, 99)
    return [
        f"def compute_{index}(base: int, scale: float, label: str) -> float:",
        f'    """Deterministic arithmetic body {index}."""',
        f"    offset: int = {a}",
        f"    factor: float = {b}.5",
        "    total = base * scale + offset * factor",
        f'    tag: str = label + "_{index}"',
        "    if total > 0:",
        "        return total",
        "    return float(len(tag))",
        "",
        "",
    ]


def gen_class(rng: random.Random, index: int) -> list[str]:
    """One class with annotated attributes and two methods (~16 lines)."""
    default = rng.randint(1, 9)
    return [
        f"class Record{index}:",
        f'    """Synthetic record {index}."""',
        "",
        f"    def __init__(self, key: str, count: int = {default}) -> None:",
        "        self.key = key",
        "        self.count = count",
        "        self.values: list[int] = []",
        "",
        "    def add(self, value: int) -> None:",
        "        self.values.append(value + self.count)",
        "",
        "    def summary(self) -> dict[str, int]:",
        '        return {"count": self.count, "total": sum(self.values)}',
        "",
        "",
    ]


def gen_constants(rng: random.Random, index: int) -> list[str]:
    """Module-level definitions: literals, containers, comprehensions."""
    n = rng.randint(3, 9)
    return [
        f"LIMIT_{index}: int = {rng.randint(10, 500)}",
        f'NAME_{index} = "record_{index}"',
        f"WEIGHTS_{index}: list[float] = [{rng.randint(1, 9)}.0, {rng.randint(1, 9)}.5]",
        f'TABLE_{index}: dict[str, int] = {{"a": {n}, "b": {n + 1}}}',
        f"SQUARES_{index} = [i * i for i in range({n})]",
        f"ALIAS_{index} = LIMIT_{index}",
        "",
    ]


def gen_module(rng: random.Random, index: int, files: int, loc_target: int) -> str:
    """One module of roughly `loc_target` lines."""
    lines: list[str] = [MODULE_TEMPLATE_HEADER.format(index=index)]
    if index > 0:
        lines.append(
            f"from mod_{(index - 1) % files:05d} import compute_0  # noqa: F401"
        )
    lines.append("")
    block = 0
    while len(lines) < loc_target:
        lines.extend(gen_constants(rng, block))
        lines.extend(gen_function(rng, block))
        lines.extend(gen_class(rng, block))
        block += 1
    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out_dir", type=Path)
    parser.add_argument("--files", type=int, default=2100)
    parser.add_argument("--loc-per-file", type=int, default=500)
    parser.add_argument("--seed", type=int, default=20260718)
    args = parser.parse_args()

    args.out_dir.mkdir(parents=True, exist_ok=True)
    rng = random.Random(args.seed)
    total_loc = 0
    for index in range(args.files):
        source = gen_module(rng, index, args.files, args.loc_per_file)
        total_loc += source.count("\n")
        (args.out_dir / f"mod_{index:05d}.py").write_text(source, encoding="utf-8")
    print(f"wrote {args.files} files, {total_loc} LOC to {args.out_dir}")


if __name__ == "__main__":
    main()
