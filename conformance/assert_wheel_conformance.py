#!/usr/bin/env python3
# Implements [CHKARCH-CONFORMANCE]. The pristine-fixture regression gate: fail
# unless every file the frozen python/typing harness graded is a Pass with zero
# false positives. It reads the harness's OWN `results/basilisk/*.toml` and does
# no scoring of its own. conformance/run_conformance.py invokes it after running
# the suite against the compiled binary; the release workflow also invokes it
# against the pip-installed wheel. Passing is internal regression evidence only;
# [CHKARCH-CONFORMANCE-MUTATION] and off-suite tests cover fixture fitting.
"""Fail unless every scored file in the frozen upstream fixture set is a Pass.

Reads the `results/basilisk/*.toml` files written by `src/main.py` at
python/typing's last Basilisk-adapter revision after it ran against a binary or
installed wheel, and enforces:

  * conformance_automated == "Pass" for every scored file (empty errors_diff),
  * zero unexpected diagnostics (false positives) across the suite,
  * at least `--min-files` graded (guards against a broken/empty run silently
    "passing" — the exact failure mode the ratchet is meant to catch).

The repo-wide conformance ratchet was deleted from coverage-thresholds.json on
2026-08-08 at the user's direction (see its _conformance_gate_removed note) —
conformance is a regression indicator, never a build gate. This script's
built-in defaults (threshold 100, max false positives 0) apply only when it is
invoked explicitly; nothing in `make test` or CI calls it any more.

The percentage here is an internal threshold over frozen fixtures. It is not a
current official conformance score and must not be published as one.

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
        print(
            "usage: assert_wheel_conformance.py <results/basilisk dir>", file=sys.stderr
        )
        return 2

    results_dir = Path(opts["dir"])
    tomls = sorted(p for p in results_dir.glob("*.toml") if p.name != "version.toml")
    if not tomls:
        print(
            f"✗ no result .toml files in {results_dir} — harness did not run",
            file=sys.stderr,
        )
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
            first = (
                diff.splitlines()[0] if diff else f"conformance_automated={automated}"
            )
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
    print("  PRISTINE FIXTURE REGRESSION GATE — frozen python/typing fixtures")
    print(f"  version:  {version or '(unknown)'}")
    print(f"  files:    {total} graded | {passed} pass | {total - passed} fail")
    print(f"  fixture pass rate: {pct}%   false positives: {false_positives}")
    print("=" * 64)

    ok = True
    if total < opts["min_files"]:
        print(
            f"✗ only {total} files graded (< {opts['min_files']}) — run looks broken",
            file=sys.stderr,
        )
        ok = False
    if pct < opts["threshold"]:
        ok = False
        print(f"✗ {pct}% < {opts['threshold']}% threshold", file=sys.stderr)
        for line in failures[:25]:
            print(f"    FAIL {line}", file=sys.stderr)
    if false_positives > opts["max_fp"]:
        ok = False
        print(
            f"✗ {false_positives} false positives > {opts['max_fp']} ceiling",
            file=sys.stderr,
        )
    if ok:
        print(
            f"✓ fixture regression passes: {passed}/{total}, "
            f"{false_positives} false positives — gate PASS"
        )
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
