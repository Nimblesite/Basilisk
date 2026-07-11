#!/usr/bin/env python3
"""Classify a Hyperfine process measurement as stable or scheduler-noisy."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Mapping

NOISY_EXIT = 10
DEFAULT_MAX_CV = 0.15


def coefficient_of_variation(result: Mapping[str, object]) -> float:
    """Return standard deviation / mean; invalid non-positive means are noisy."""
    mean = float(result["mean"])
    stddev = float(result["stddev"])
    if not math.isfinite(mean) or not math.isfinite(stddev) or mean <= 0 or stddev < 0:
        return math.inf
    return stddev / mean


def measurement_is_noisy(result: Mapping[str, object], *, max_cv: float) -> bool:
    """Whether a measurement needs more samples before it can move the ratchet."""
    return coefficient_of_variation(result) > max_cv


def load_measurement(path: Path, command: str) -> Mapping[str, object]:
    data = json.loads(path.read_text(encoding="utf-8"))
    for result in data.get("results", []):
        if result.get("command") == command:
            return result
    raise ValueError(f"{path}: no Hyperfine result named {command!r}")


def main(argv: list[str]) -> int:
    if not 2 <= len(argv) <= 3:
        print(
            "usage: stability.py <hyperfine.json> <max-cv> [command]", file=sys.stderr
        )
        return 2

    path = Path(argv[0])
    try:
        max_cv = float(argv[1])
        if not 0 < max_cv < 1:
            raise ValueError("max-cv must be between 0 and 1")
        command = argv[2] if len(argv) == 3 else "basilisk"
        result = load_measurement(path, command)
        cv = coefficient_of_variation(result)
    except (OSError, ValueError, TypeError, KeyError, json.JSONDecodeError) as exc:
        print(f"ERROR: cannot validate benchmark stability: {exc}", file=sys.stderr)
        return 2

    print(f"{command} coefficient of variation: {cv:.1%} (limit {max_cv:.1%})")
    return NOISY_EXIT if cv > max_cv else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
