#!/usr/bin/env python3
"""Fail if any public surface says something [WITHDRAWAL-PROHIBITED] forbids.

Implements [WITHDRAWAL-SURFACES]. `scripts/test_published_readmes.py` proves the
five generated storefront READMEs match the messaging spec; nothing proved
anything about the rest — the crate READMEs, the security policy, the package
manifests, the store descriptions, the website templates. Those are public too,
and a marketing sentence or a stale "shipping" claim in one of them contradicts
the statement just as loudly as one in a README.

    python3 scripts/check_public_copy.py            # scan; non-zero on a hit
    python3 scripts/check_public_copy.py --list     # print the scanned surfaces

Every rule below cites the spec bullet it enforces. Add a rule when the spec
gains a prohibition — never an exemption for a surface that trips one.
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SPEC = "docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md"

# Everything a stranger can read without cloning: listings, storefronts, the
# site, and the files GitHub renders on the repository page. Internal specs,
# plans and the integrity audit are deliberately absent — they are the record,
# not marketing surfaces ([WITHDRAWAL-UNLIST]).
PUBLIC_SURFACES: tuple[str, ...] = (
    "README.md",
    "README-pypi.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "pyproject.toml",
    "crates/*/README.md",
    "vscode-extension/README.md",
    "vscode-extension/package.json",
    "basilisk.nvim/README.md",
    "basilisk.nvim/doc/basilisk.txt",
    "basilisk-zed/README.md",
    "basilisk-zed/extension.toml",
    ".github/release-templates/*.tmpl",
    "website/src/*.njk",
    "website/src/_includes/**/*.njk",
    "delist/README.md",
    "book/README.md",
)

APOLOGY = "christianfindlay.com/blog/basilisk-conformance-apology"


@dataclass(frozen=True)
class Rule:
    """One prohibition, as a pattern plus the reason it exists."""

    id: str
    pattern: re.Pattern[str]
    why: str


RULES: tuple[Rule, ...] = (
    Rule(
        "measured-figure",
        re.compile(r"\b\d{1,3}(?:\.\d+)?\s?%"),
        "no conformance or benchmark figure, in any tense, caveated or archived",
    ),
    # A figure, not the word: the approved copy says "removed from the
    # python/typing conformance results" and links PR #2330, so `conformance`
    # and a bare number are both allowed. A percentage, a score, or an "N of M"
    # is not.
    Rule(
        "conformance-score",
        re.compile(
            r"(?:conformance|benchmark|pass rate|score)[^.\n]{0,80}?"
            r"\b\d{1,3}(?:\.\d+)?\s?(?:%|of\s+\d|/\s?\d)|"
            r"\bscored?\b[^.\n]{0,40}?\d",
            re.IGNORECASE,
        ),
        "no conformance or benchmark figure, in any tense, caveated or archived",
    ),
    Rule(
        "install-instruction",
        re.compile(
            r"\b(pip|uv tool|uv|brew|scoop|cargo|npm|npx|pipx)\s+install\s+\S*basilisk",
            re.IGNORECASE,
        ),
        "no install instructions",
    ),
    Rule(
        "rule-count",
        re.compile(r"\b\d+\+?\s+(rules|diagnostics|checks|lints)\b", re.IGNORECASE),
        "no feature marketing or rule counts",
    ),
    Rule(
        "feature-marketing",
        re.compile(
            r"\b(strict[- ]by[- ]default|blazing|blazingly|lightning[- ]fast|"
            r"fastest|best[- ]in[- ]class|production[- ]ready|batteries[- ]included|"
            r"drop[- ]in replacement|just works)\b",
            re.IGNORECASE,
        ),
        "no feature marketing",
    ),
    Rule(
        "scoping-reassurance",
        re.compile(
            r"\b(only a (few|handful)|small number of rules|"
            r"(is|are|remains?) unaffected|safe to (keep )?us(e|ing)|"
            r"keep using|still (safe|fine|works? fine))\b",
            re.IGNORECASE,
        ),
        "no scoping reassurance — we cannot scope it, and saying so is the point",
    ),
    Rule(
        "shipping-claim",
        re.compile(
            r"^\s*(Working|Complete|Shipped|Shipping|Stable|Ready)\s*[-—–:]",
            re.IGNORECASE | re.MULTILINE,
        ),
        "no claim that something is shipped — nothing ships but the statement",
    ),
    Rule(
        "quoted-apology",
        re.compile(rf"^\s*>.*{re.escape(APOLOGY)}", re.MULTILINE),
        "never quote the apology — link it, neutrally, and nothing more",
    ),
)


def surfaces() -> list[Path]:
    """Every public file, resolved and de-duplicated, in a stable order."""
    found: set[Path] = set()
    for pattern in PUBLIC_SURFACES:
        found.update(path for path in REPO_ROOT.glob(pattern) if path.is_file())
    return sorted(found)


def scan(path: Path) -> list[tuple[Rule, str]]:
    """Every prohibition `path` trips, with the offending text."""
    text = path.read_text(encoding="utf-8")
    hits: list[tuple[Rule, str]] = []
    for rule in RULES:
        match = rule.pattern.search(text)
        if match:
            hits.append((rule, match.group(0).strip()))
    return hits


def report(path: Path, hits: list[tuple[Rule, str]]) -> None:
    """Print one surface's failures the way a reviewer needs to read them."""
    relative = path.relative_to(REPO_ROOT)
    for rule, offending in hits:
        print(f"error: {relative}: [{rule.id}] {rule.why}", file=sys.stderr)
        print(f"       matched: {offending!r}", file=sys.stderr)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--list", action="store_true", help="print the scanned surfaces and exit"
    )
    args = parser.parse_args()

    paths = surfaces()
    if args.list:
        for path in paths:
            print(path.relative_to(REPO_ROOT))
        return 0

    failed = False
    for path in paths:
        hits = scan(path)
        if hits:
            report(path, hits)
            failed = True

    if failed:
        print(
            f"\nThe prohibitions are {SPEC} [WITHDRAWAL-PROHIBITED].", file=sys.stderr
        )
        return 1
    print(f"✓ {len(paths)} public surfaces carry no prohibited copy")
    return 0


if __name__ == "__main__":
    sys.exit(main())
