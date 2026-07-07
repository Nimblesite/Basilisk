#!/usr/bin/env python3
# Implements [CHKARCH-CONFORMANCE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md
"""Stamp the live conformance score + graded commit into the README and spec.

Static docs (README.md, README.zh.md, the checker-architecture spec) quote the
conformance score and the exact `python/typing` commit it was measured against.
Those drift as the checker improves and `main` advances. This generator reads
`website/src/_data/conformance_report.json` — written by `conformance/score.py`
on every run — and refreshes the quoted values in place, so the docs can never
silently contradict the self-measured number.

It updates two kinds of spot, both render-safe (the markers are invisible HTML
comments, so they work mid-sentence, inside a list item, or inside a table cell):

  • inline markers   `<!--g:NAME-->value<!--/g:NAME-->`  -> the value for NAME
  • commit-tree URLs `github.com/python/typing/tree/<sha>/conformance` -> the sha

Usage:
    python3 scripts/gen_conformance_reference.py            # rewrite in place
    python3 scripts/gen_conformance_reference.py --check    # CI: fail if stale
"""

from __future__ import annotations

import json
import math
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPORT = ROOT / "website" / "src" / "_data" / "conformance_report.json"
BENCH_STATUS_DIR = ROOT / "benchmarks" / "status"
TARGETS = (
    ROOT / "README.md",
    ROOT / "README.zh.md",
    ROOT / "docs" / "specs" / "CHECKER-ARCHITECTURE-SPEC.md",
    # The VS Code marketplace READMEs boast the same score — keep them in lock
    # step so the published listing can never quote a stale number.
    ROOT / "vscode-extension" / "README.md",
    ROOT / "vscode-extension" / "README.zh.md",
)

# The checkers whose median cold time the README bench table quotes. Key is the
# CSV `<tool>_ms` column; the sentinel name is `bench<Capitalized>` (e.g.
# `benchBasilisk`), stamped inline in the table cell so it never breaks the
# markdown table the way a standalone comment line would.
BENCH_TOOLS = ("basilisk", "pyright", "mypy", "ty", "pyrefly", "zuban")

MARKER_RE = re.compile(r"<!--g:(?P<name>[A-Za-z]+)-->.*?<!--/g:(?P=name)-->", re.S)
TREE_SHA_RE = re.compile(
    r"(github\.com/python/typing/tree/)[0-9a-fA-F]{7,40}(/conformance)"
)


def values(report: dict) -> dict[str, str]:
    """The named values the markers may reference, from the score report."""
    score = report["score"]
    upstream = report["upstream"]
    return {
        "score": f"{score['scorePct']}%",
        "pass": str(score["pass"]),
        "total": str(score["total"]),
        "fp": str(score["falsePositives"]),
        "missed": str(score["missed"]),
        "caught": str(score["caught"]),
        "short": upstream["shortSha"],
    }


def _median_ms(nums: list[float]) -> int | None:
    """Median of `nums`, rounded half-up to match the website's JS `Math.round`."""
    ordered = sorted(nums)
    n = len(ordered)
    if n == 0:
        return None
    mid = n // 2
    val = ordered[mid] if n % 2 else (ordered[mid - 1] + ordered[mid]) / 2
    return math.floor(val + 0.5)


def _primary_bench_csv() -> Path | None:
    """The benchmark CSV the site treats as primary: the `.primary` pin first,
    then the alphabetically-first machine — matching `_data/benchmarks.js`."""
    pin = BENCH_STATUS_DIR / ".primary"
    if pin.exists():
        csv = BENCH_STATUS_DIR / f"{pin.read_text(encoding='utf-8').strip()}.csv"
        if csv.exists():
            return csv
    csvs = sorted(BENCH_STATUS_DIR.glob("*.csv"))
    return csvs[0] if csvs else None


def bench_values() -> dict[str, str]:
    """Median cold check per tool + machine/count, read from the primary bench
    CSV so the README table can never be a hand-typed figure. Empty when no CSV
    exists (the markers are then left untouched, exactly like a missing score)."""
    csv = _primary_bench_csv()
    if csv is None:
        return {}
    cpu, header, rows = "", None, []
    for raw in csv.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line:
            continue
        if line.startswith("#"):
            body = line[1:].strip()
            if body.startswith("cpu:"):
                cpu = body.split(":", 1)[1].strip()
            continue
        parts = line.split(",")
        if header is None:
            header = parts
        else:
            rows.append(parts)
    if not header or not rows:
        return {}

    col = {name[:-3] if name.endswith("_ms") else name: i for i, name in enumerate(header)}

    def median_for(tool: str) -> int | None:
        i = col.get(tool)
        if i is None:
            return None
        nums = []
        for r in rows:
            if i < len(r) and r[i]:
                try:
                    nums.append(float(r[i]))
                except ValueError:
                    pass
        return _median_ms(nums)

    vals: dict[str, str] = {}
    for tool in BENCH_TOOLS:
        m = median_for(tool)
        if m is not None:
            vals[f"bench{tool.capitalize()}"] = str(m)
    warm = median_for("basilisk-warm")
    if warm is not None:
        vals["benchWarm"] = str(warm)
    if cpu:
        vals["benchMachine"] = cpu
    vals["benchCount"] = str(len(rows))
    return vals


def stamp(text: str, vals: dict[str, str]) -> str:
    """Refresh every inline marker and every commit-tree URL in `text`."""

    def marker(match: re.Match[str]) -> str:
        name = match.group("name")
        value = vals.get(name)
        if value is None:
            return match.group(0)  # unknown marker — leave it untouched
        return f"<!--g:{name}-->{value}<!--/g:{name}-->"

    text = MARKER_RE.sub(marker, text)
    return TREE_SHA_RE.sub(lambda m: f"{m.group(1)}{vals['sha']}{m.group(2)}", text)


def main(argv: list[str]) -> int:
    check = "--check" in argv
    if not REPORT.exists():
        print(
            f"  ✗ {REPORT.relative_to(ROOT)} not found — run conformance/score.py first",
            file=sys.stderr,
        )
        return 1

    report = json.loads(REPORT.read_text(encoding="utf-8"))
    vals = values(report)
    vals["sha"] = report["upstream"]["sha"]  # full sha for the tree URLs only
    vals.update(bench_values())  # median cold check per tool, from the primary CSV

    stale: list[Path] = []
    for path in TARGETS:
        if not path.exists():
            continue
        original = path.read_text(encoding="utf-8")
        updated = stamp(original, vals)
        if updated != original:
            stale.append(path)
            if not check:
                path.write_text(updated, encoding="utf-8")

    if check:
        if stale:
            print(
                "  ✗ conformance docs are stale — run "
                "scripts/gen_conformance_reference.py:",
                file=sys.stderr,
            )
            for path in stale:
                print(f"    - {path.relative_to(ROOT)}", file=sys.stderr)
            return 1
        print("  conformance docs up to date.")
        return 0

    if stale:
        print(f"  Stamped conformance {vals['score']} (commit {vals['short']}) into:")
        for path in stale:
            print(f"    - {path.relative_to(ROOT)}")
    else:
        print("  conformance docs already up to date.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
