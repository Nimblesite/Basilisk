#!/usr/bin/env python3
"""Generate the canonical diagnostic-code reference from the checker source.

Single source of truth: the `//! BSK-E####: <description>` (and `BSK-W####`)
header on each rule module under crates/basilisk-checker/src/rules/.

Usage:
    python3 scripts/gen_rules_reference.py            # print a Markdown table
    python3 scripts/gen_rules_reference.py --json      # emit JSON
    python3 scripts/gen_rules_reference.py --check FILE # verify FILE contains
                                                        # every current code

Run this after adding or renaming a rule, and paste the table into
website/src/docs/rules/index.md (between the REFERENCE markers).
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RULES_DIR = ROOT / "crates" / "basilisk-checker" / "src" / "rules"

HEADER = re.compile(r"//!\s*(BSK-[EW]\d{4}):\s*(.*)")
CONT = re.compile(r"//!\s*(.*)")


def clean(text: str) -> str:
    text = text.strip().rstrip(".").strip()
    # collapse internal whitespace
    return re.sub(r"\s+", " ", text)


def extract() -> dict[str, str]:
    """Map code -> description, taking the first header found per code."""
    codes: dict[str, str] = {}
    for path in sorted(RULES_DIR.rglob("*.rs")):
        lines = path.read_text(encoding="utf-8").splitlines()
        for i, line in enumerate(lines):
            m = HEADER.match(line.strip())
            if not m:
                continue
            code, desc = m.group(1), m.group(2)
            # Stitch a wrapped description (next //! line) when the first
            # line ends without sentence-final punctuation.
            if desc and not desc.rstrip().endswith((".", "!", ")")):
                nxt = CONT.match(lines[i + 1].strip()) if i + 1 < len(lines) else None
                if nxt and not HEADER.match(lines[i + 1].strip()):
                    desc = f"{desc} {nxt.group(1)}"
            codes.setdefault(code, clean(desc))
    return codes


def sort_key(code: str) -> tuple[int, int]:
    # E before W, then numeric.
    return (0 if code[4] == "E" else 1, int(code[5:]))


def to_markdown(codes: dict[str, str]) -> str:
    rows = ["| Code | Description |", "|---|---|"]
    for code in sorted(codes, key=sort_key):
        rows.append(f"| `{code}` | {codes[code]} |")
    return "\n".join(rows)


def main() -> int:
    codes = extract()
    if "--json" in sys.argv:
        print(json.dumps(codes, indent=2, sort_keys=True))
        return 0
    if "--check" in sys.argv:
        target = Path(sys.argv[sys.argv.index("--check") + 1]).read_text(
            encoding="utf-8"
        )
        missing = [c for c in codes if c not in target]
        if missing:
            print(
                f"MISSING {len(missing)} codes: {', '.join(sorted(missing, key=sort_key))}"
            )
            return 1
        print(f"OK: all {len(codes)} codes present")
        return 0
    print(to_markdown(codes))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
