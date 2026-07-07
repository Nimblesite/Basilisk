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
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPORT = ROOT / "website" / "src" / "_data" / "conformance_report.json"
TARGETS = (
    ROOT / "README.md",
    ROOT / "README.zh.md",
    ROOT / "docs" / "specs" / "CHECKER-ARCHITECTURE-SPEC.md",
    # The VS Code marketplace READMEs boast the same score — keep them in lock
    # step so the published listing can never quote a stale number.
    ROOT / "vscode-extension" / "README.md",
    ROOT / "vscode-extension" / "README.zh.md",
)

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
