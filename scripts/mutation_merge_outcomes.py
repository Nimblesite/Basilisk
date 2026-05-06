#!/usr/bin/env python3
"""Merge cargo-mutants shard outcomes into one cargo-mutants outcomes file."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def load_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise FileNotFoundError(path)
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict) or not isinstance(data.get("outcomes"), list):
        raise ValueError(f"invalid cargo-mutants outcomes file: {path}")
    return data


def mutant(outcome: dict[str, Any]) -> dict[str, Any] | None:
    scenario = outcome.get("scenario")
    if not isinstance(scenario, dict):
        return None
    raw = scenario.get("Mutant")
    return raw if isinstance(raw, dict) else None


def sort_key(outcome: dict[str, Any]) -> tuple[Any, ...]:
    raw = mutant(outcome)
    if raw is None:
        return ("", 0, 0, "", "", "")
    span = raw.get("span", {})
    start = span.get("start", {}) if isinstance(span, dict) else {}
    function = raw.get("function", {})
    return (
        raw.get("file", ""),
        start.get("line", 0),
        start.get("column", 0),
        function.get("function_name", "") if isinstance(function, dict) else "",
        raw.get("genre", ""),
        raw.get("replacement", ""),
    )


def tally(outcomes: list[dict[str, Any]]) -> dict[str, int]:
    counts = {"caught": 0, "missed": 0, "timeout": 0, "unviable": 0, "total": 0}
    for outcome in outcomes:
        if mutant(outcome) is None:
            continue
        counts["total"] += 1
        match outcome.get("summary"):
            case "CaughtMutant":
                counts["caught"] += 1
            case "MissedMutant":
                counts["missed"] += 1
            case "Timeout":
                counts["timeout"] += 1
            case "Unviable":
                counts["unviable"] += 1
    return counts


def merge(paths: list[Path]) -> dict[str, Any]:
    loaded = [load_json(path) for path in paths]
    outcomes = [
        outcome
        for data in loaded
        for outcome in data["outcomes"]
        if isinstance(outcome, dict)
    ]
    outcomes.sort(key=sort_key)

    starts = [data.get("start_time") for data in loaded if data.get("start_time")]
    ends = [data.get("end_time") for data in loaded if data.get("end_time")]
    counts = tally(outcomes)

    merged = dict(loaded[0])
    merged["outcomes"] = outcomes
    merged["total_mutants"] = counts["total"]
    merged["caught"] = counts["caught"]
    merged["missed"] = counts["missed"]
    merged["timeout"] = counts["timeout"]
    merged["unviable"] = counts["unviable"]
    if starts:
        merged["start_time"] = min(starts)
    if ends:
        merged["end_time"] = max(ends)
    return merged


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("outcomes", nargs="+", type=Path)
    parser.add_argument("-o", "--output", required=True, type=Path)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        merged = merge(args.outcomes)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"failed to merge mutation outcomes: {error}", file=sys.stderr)
        return 1

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(merged, indent=2, sort_keys=True) + "\n")
    counts = tally(merged["outcomes"])
    print(
        "Merged mutation outcomes: "
        f"total={counts['total']} caught={counts['caught']} "
        f"missed={counts['missed']} timeout={counts['timeout']} "
        f"unviable={counts['unviable']}",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
