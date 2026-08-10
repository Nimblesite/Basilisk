#!/usr/bin/env python3
"""Extract the approved withdrawal copy from the messaging spec into site data.

Implements [WITHDRAWAL-COPY]. The single source of truth for everything Basilisk
says publicly is docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md; this script lifts
its approved copy blocks out into website/src/_data/withdrawal.json so the site
renders the spec's words rather than a hand-typed copy of them. `copy_blocks()`
serves the same text as markdown to scripts/gen_readmes.py, so the site and every
published README are two renderings of one source.

    python3 scripts/gen_withdrawal_copy.py            # write the data file
    python3 scripts/gen_withdrawal_copy.py --check    # fail if it has drifted

Run --check in CI: the site must never say something the spec does not.
"""

from __future__ import annotations

import argparse
import html
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SPEC_PATH = REPO_ROOT / "docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md"
DATA_PATH = REPO_ROOT / "website/src/_data/withdrawal.json"
# The CLI and the extension print the SAME bytes the spec authored: both read a
# generated file rather than a hand-typed string ([WITHDRAWAL-INERT-TEXT]).
CLI_NOTICE_PATH = REPO_ROOT / "crates/basilisk-cli/src/withdrawal_notice.txt"
VSIX_NOTICE_PATH = REPO_ROOT / "vscode-extension/src/withdrawal-notice.ts"

# The anchors naming each approved block in the spec.
ANCHOR_LINE = "{#WITHDRAWAL-COPY-LINE}"
ANCHOR_SHORT = "{#WITHDRAWAL-COPY-SHORT}"
ANCHOR_ACTION = "{#WITHDRAWAL-COPY-ACTION}"
ANCHOR_FULL = "{#WITHDRAWAL-COPY-FULL}"
ANCHOR_NOTICE = "{#WITHDRAWAL-INERT-TEXT}"

CODE_RE = re.compile(r"`([^`]+)`")
LINK_RE = re.compile(r"\[([^\]]+)\]\(([^)]+)\)")
BOLD_RE = re.compile(r"\*\*([^*]+)\*\*")
ITALIC_RE = re.compile(r"\*([^*]+)\*")


class SpecError(RuntimeError):
    """The spec is missing a block this script is required to publish."""


def blockquote_after(lines: list[str], anchor: str) -> list[str]:
    """Return the paragraphs of the blockquote following `anchor`.

    Paragraphs are joined to one line each: the spec is authored unwrapped, but a
    blockquote may still carry several paragraphs separated by a bare `>`.
    """
    try:
        start = next(i for i, line in enumerate(lines) if anchor in line)
    except StopIteration:
        raise SpecError(f"{SPEC_PATH.name} has no {anchor} block") from None

    cursor = start + 1
    while cursor < len(lines) and not lines[cursor].strip():
        cursor += 1

    quoted: list[str] = []
    while cursor < len(lines) and lines[cursor].startswith(">"):
        quoted.append(lines[cursor].removeprefix(">").strip())
        cursor += 1

    if not quoted:
        raise SpecError(f"{anchor} in {SPEC_PATH.name} is not followed by a blockquote")

    # A bare `>` closes a paragraph; consecutive text lines join, so the block
    # survives an editor re-wrapping the spec.
    paragraphs: list[str] = []
    open_paragraph = False
    for chunk in quoted:
        if not chunk:
            open_paragraph = False
        elif open_paragraph:
            paragraphs[-1] = f"{paragraphs[-1]} {chunk}"
        else:
            paragraphs.append(chunk)
            open_paragraph = True
    return paragraphs


def fenced_after(lines: list[str], anchor: str) -> str:
    """Return the fenced code block following `anchor`, verbatim.

    This is the text the inert CLI and the extension print, so it is lifted
    byte-for-byte: no wrapping, no markdown, no substitution.
    """
    try:
        start = next(i for i, line in enumerate(lines) if anchor in line)
    except StopIteration:
        raise SpecError(f"{SPEC_PATH.name} has no {anchor} block") from None

    opened = False
    body: list[str] = []
    for line in lines[start + 1 :]:
        if line.startswith("```"):
            if opened:
                return "\n".join(body) + "\n"
            opened = True
        elif opened:
            body.append(line)
    raise SpecError(f"{anchor} in {SPEC_PATH.name} has no closing code fence")


def to_html(markdown: str) -> str:
    """Render the inline markdown the approved copy uses, and nothing else.

    Escaping runs first so the spec's text can never inject markup; the patterns
    below then reintroduce exactly the four inline constructs the copy contains.
    """
    text = html.escape(markdown, quote=False)
    text = CODE_RE.sub(r"<code>\1</code>", text)
    text = LINK_RE.sub(r'<a href="\2">\1</a>', text)
    text = BOLD_RE.sub(r"<strong>\1</strong>", text)
    return ITALIC_RE.sub(r"<em>\1</em>", text)


@dataclass(frozen=True)
class Copy:
    """The approved blocks, as the markdown the spec authored.

    Each consumer renders this for its own medium: the site converts to HTML,
    the published READMEs use the markdown unchanged.
    """

    line: str
    title: str
    short: tuple[str, ...]
    action: tuple[str, ...]
    full: tuple[str, ...]


def copy_blocks() -> Copy:
    """Extract every approved block from the messaging spec."""
    lines = SPEC_PATH.read_text(encoding="utf-8").splitlines()

    one_line = blockquote_after(lines, ANCHOR_LINE)
    if len(one_line) != 1:
        raise SpecError(f"{ANCHOR_LINE} must be exactly one paragraph")

    full = blockquote_after(lines, ANCHOR_FULL)
    if not full or not full[0].startswith("# "):
        raise SpecError(f"{ANCHOR_FULL} must open with a level-1 heading")

    return Copy(
        line=one_line[0],
        title=full[0].removeprefix("# ").strip(),
        short=tuple(blockquote_after(lines, ANCHOR_SHORT)),
        action=tuple(blockquote_after(lines, ANCHOR_ACTION)),
        full=tuple(full[1:]),
    )


def build() -> dict[str, object]:
    """Assemble the site data payload from the spec's approved blocks."""
    copy = copy_blocks()
    return {
        "_generated": f"Generated from {SPEC_PATH.relative_to(REPO_ROOT)} "
        "by scripts/gen_withdrawal_copy.py — DO NOT EDIT.",
        "line": copy.line,
        "title": copy.title,
        "short": [to_html(p) for p in copy.short],
        "action": [to_html(p) for p in copy.action],
        "full": [to_html(p) for p in copy.full],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the data file matches the spec instead of writing it",
    )
    args = parser.parse_args()

    try:
        payload = json.dumps(build(), indent=2, ensure_ascii=False) + "\n"
    except SpecError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    if not args.check:
        DATA_PATH.write_text(payload, encoding="utf-8")
        print(f"wrote {DATA_PATH.relative_to(REPO_ROOT)}")
        return 0

    current = DATA_PATH.read_text(encoding="utf-8") if DATA_PATH.exists() else ""
    if current == payload:
        return 0
    print(
        f"error: {DATA_PATH.relative_to(REPO_ROOT)} has drifted from "
        f"{SPEC_PATH.relative_to(REPO_ROOT)}. Run: python3 scripts/gen_withdrawal_copy.py",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
