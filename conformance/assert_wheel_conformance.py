#!/usr/bin/env python3
# Implements [CHKARCH-CONFORMANCE]. Asserts the pip-installed `basilisk-python`
# wheel passes the REAL python/typing harness. This is the release/CI gate for
# the shipped artifact — it does NOT replace conformance/score.py (the sha-pinned
# fast local scorer + anti-gaming gate), it proves the WHEEL a user installs
# scores identically via the suite's own unmodified `src/main.py`.
"""Fail unless every scored file the upstream harness graded is a Pass.

Reads the `results/basilisk/*.toml` files written by `src/main.py` (upstream's
harness) after it ran against the installed wheel, and enforces:

  * conformance_automated == "Pass" for every scored file (empty errors_diff),
  * zero unexpected diagnostics (false positives) across the suite,
  * at least `--min-files` graded (guards against a broken/empty run silently
    "passing" — the exact failure mode the ratchet is meant to catch).

Thresholds default to the repo ratchet in coverage-thresholds.json
(conformance.threshold = 100%, conformance.max_false_positives = 0).

Usage:
    python3 conformance/assert_wheel_conformance.py <results/basilisk dir>
        [--min-files N] [--threshold PCT] [--max-false-positives N]
"""

from __future__ import annotations

import json
import sys
import tomllib
from pathlib import Path


def repo_root() -> Path:
    here = Path(__file__).resolve()
    for parent in here.parents:
        if (parent / "Cargo.toml").exists() and (parent / "crates").exists():
            return parent
    return here.parent.parent


def ratchet(key: str, default: int) -> int:
    try:
        data = json.loads((repo_root() / "coverage-thresholds.json").read_text())
        return int(data["conformance"][key])
    except (OSError, KeyError, ValueError, json.JSONDecodeError):
        return default


def parse_opts(argv: list[str]) -> dict:
    opts = {
        "dir": None,
        "min_files": 100,
        "threshold": ratchet("threshold", 100),
        "max_fp": ratchet("max_false_positives", 0),
    }
    it = iter(argv)
    for arg in it:
        if arg == "--min-files":
            opts["min_files"] = int(next(it))
        elif arg == "--threshold":
            opts["threshold"] = int(next(it))
        elif arg == "--max-false-positives":
            opts["max_fp"] = int(next(it))
        elif opts["dir"] is None:
            opts["dir"] = arg
    return opts


def main(argv: list[str]) -> int:
    opts = parse_opts(argv)
    if opts["dir"] is None:
        print("usage: assert_wheel_conformance.py <results/basilisk dir>", file=sys.stderr)
        return 2

    results_dir = Path(opts["dir"])
    tomls = sorted(p for p in results_dir.glob("*.toml") if p.name != "version.toml")
    if not tomls:
        print(f"✗ no result .toml files in {results_dir} — harness did not run", file=sys.stderr)
        return 1

    failures: list[str] = []
    false_positives = 0
    passed = 0
    for toml_path in tomls:
        try:
            data = tomllib.loads(toml_path.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError) as exc:
            failures.append(f"{toml_path.name}: unreadable ({exc})")
            continue
        automated = data.get("conformance_automated", "Fail")
        diff = (data.get("errors_diff") or "").strip()
        false_positives += sum(1 for ln in diff.splitlines() if "Unexpected" in ln)
        if automated == "Pass" and not diff:
            passed += 1
        else:
            first = diff.splitlines()[0] if diff else f"conformance_automated={automated}"
            failures.append(f"{toml_path.name}: {first}")

    total = len(tomls)
    pct = passed * 100 // total if total else 0
    version = ""
    version_file = results_dir / "version.toml"
    if version_file.exists():
        try:
            version = tomllib.loads(version_file.read_text()).get("version", "")
        except (OSError, tomllib.TOMLDecodeError):
            version = ""

    print("=" * 64)
    print("  WHEEL CONFORMANCE GATE — pip-installed basilisk vs python/typing")
    print(f"  version:  {version or '(unknown)'}")
    print(f"  files:    {total} graded | {passed} pass | {total - passed} fail")
    print(f"  score:    {pct}%   false positives: {false_positives}")
    print("=" * 64)

    ok = True
    if total < opts["min_files"]:
        print(f"✗ only {total} files graded (< {opts['min_files']}) — run looks broken", file=sys.stderr)
        ok = False
    if pct < opts["threshold"]:
        ok = False
        print(f"✗ {pct}% < {opts['threshold']}% threshold", file=sys.stderr)
        for line in failures[:25]:
            print(f"    FAIL {line}", file=sys.stderr)
    if false_positives > opts["max_fp"]:
        ok = False
        print(f"✗ {false_positives} false positives > {opts['max_fp']} ceiling", file=sys.stderr)
    if ok:
        print(f"✓ wheel passes {passed}/{total} ({pct}%), {false_positives} false positives — gate PASS")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
