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
# The CLI and every editor extension print the SAME bytes the spec authored:
# each reads a generated file rather than a hand-typed string
# ([WITHDRAWAL-INERT-TEXT]).
CLI_NOTICE_PATH = REPO_ROOT / "crates/basilisk-cli/src/withdrawal_notice.txt"
VSIX_NOTICE_PATH = REPO_ROOT / "vscode-extension/src/withdrawal-notice.ts"
NVIM_NOTICE_PATH = REPO_ROOT / "basilisk.nvim/lua/basilisk/notice.lua"
NVIM_DOC_PATH = REPO_ROOT / "basilisk.nvim/doc/basilisk.txt"
ZED_NOTICE_PATH = REPO_ROOT / "basilisk-zed/src/withdrawal_notice.txt"

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
        # The same block as markdown, for surfaces that are not HTML. llms.txt
        # is read by machines: stripping the tags out of the HTML would drop
        # the source links the copy carries, and rewriting the copy without
        # them would be a fourth variant of the message.
        "full_markdown": list(copy.full),
    }


def notice_text() -> str:
    """The exact bytes the inert CLI and the extension print."""
    return fenced_after(
        SPEC_PATH.read_text(encoding="utf-8").splitlines(), ANCHOR_NOTICE
    )


def vsix_notice_module(notice: str) -> str:
    """The notice as a TypeScript module the extension imports."""
    return (
        "// GENERATED FILE — DO NOT EDIT.\n"
        "// Source: docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md [WITHDRAWAL-INERT-TEXT]\n"
        "// Regenerate: python3 scripts/gen_withdrawal_copy.py\n"
        "/** The approved notice, verbatim. */\n"
        f"export const WITHDRAWAL_NOTICE = {json.dumps(notice)};\n"
    )


def nvim_notice_module(notice: str) -> str:
    """The notice as a Lua module the Neovim plugin requires."""
    return (
        "-- GENERATED FILE — DO NOT EDIT.\n"
        "-- Source: docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md [WITHDRAWAL-INERT-TEXT]\n"
        "-- Regenerate: python3 scripts/gen_withdrawal_copy.py\n"
        f"local text = {json.dumps(notice.rstrip(chr(10)))}\n"
        "return {\n"
        "  text = text,\n"
        '  lines = vim.split(text, "\\n", { plain = true }),\n'
        "}\n"
    )


def nvim_help_doc(notice: str) -> str:
    """`:help basilisk` — the statement, and nothing else."""
    return (
        "*basilisk.txt*  Basilisk is unlisted\n"
        "\n"
        "GENERATED FILE — DO NOT EDIT. Source:\n"
        "docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md [WITHDRAWAL-INERT-TEXT]\n"
        "\n"
        "BASILISK                                                      *basilisk*\n"
        "\n"
        f"{notice}"
        "\n"
        "vim:tw=78:ts=8:ft=help:norl:\n"
    )


def outputs() -> dict[Path, str]:
    """Every file generated from the spec, by path."""
    notice = notice_text()
    return {
        DATA_PATH: json.dumps(build(), indent=2, ensure_ascii=False) + "\n",
        CLI_NOTICE_PATH: notice,
        VSIX_NOTICE_PATH: vsix_notice_module(notice),
        NVIM_NOTICE_PATH: nvim_notice_module(notice),
        NVIM_DOC_PATH: nvim_help_doc(notice),
        # Zed compiles to WASM, so the notice is `include_str!`d like the CLI's
        # rather than escaped into a source literal.
        ZED_NOTICE_PATH: notice,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the generated files match the spec instead of writing them",
    )
    args = parser.parse_args()

    try:
        generated = outputs()
    except SpecError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    drifted = False
    for path, payload in generated.items():
        relative = path.relative_to(REPO_ROOT)
        if not args.check:
            path.write_text(payload, encoding="utf-8")
            print(f"wrote {relative}")
            continue
        if (path.read_text(encoding="utf-8") if path.exists() else "") != payload:
            print(
                f"error: {relative} has drifted from {SPEC_PATH.name}", file=sys.stderr
            )
            drifted = True
    if drifted:
        print("Run: python3 scripts/gen_withdrawal_copy.py", file=sys.stderr)
    return 1 if drifted else 0


if __name__ == "__main__":
    sys.exit(main())
