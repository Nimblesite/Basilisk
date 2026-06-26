#!/usr/bin/env python3
"""Generate the canonical diagnostic-code reference from the checker source.

Single source of truth: the `//! BSK-E####: <description>` (and `BSK-W####`)
header — and the doc-comment body beneath it — on each rule module under
crates/basilisk-checker/src/rules/.

Usage:
    python3 scripts/gen_rules_reference.py             # print a Markdown table
    python3 scripts/gen_rules_reference.py --json       # emit code->summary JSON
    python3 scripts/gen_rules_reference.py --data [OUT] # write the rich rules
                                                        # data Eleventy consumes
                                                        # (default: website/src/
                                                        # _data/rules.json)
    python3 scripts/gen_rules_reference.py --check FILE  # verify FILE contains
                                                        # every current code

The `--data` output drives both the complete reference table and the per-code
/errors/BSK-XXXX/ pages on the website, so the pages the CLI deep-links to
(`see: https://www.basilisk-python.dev/errors/BSK-EXXXX`) can never drift from
the checker. Run it after adding or renaming a rule. See
docs/specs/WEBSITE-ERROR-PAGES-SPEC.md [WEBSITE-ERROR-PAGES].
"""

from __future__ import annotations

import html
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RULES_DIR = ROOT / "crates" / "basilisk-checker" / "src" / "rules"
DEFAULT_DATA_OUT = ROOT / "website" / "src" / "_data" / "rules.json"
ERRORS_BASE_URL = "https://www.basilisk-python.dev/errors"

HEADER = re.compile(r"//!\s*(BSK-[EW]\d{4}):\s*(.*)")
DOC = re.compile(r"//!\s?(.*)")
DOCS_URL = re.compile(r'docs_url:\s*"([^"]+)"')
SPEC_REF = re.compile(r"^Implements ")

# Coarse groups for filtering/badging on the website. Errors outside the two
# foundational ranges are all part of the broader type-system surface.
GROUPS = (
    ("E", 1, 9, "Missing Annotations"),
    ("E", 10, 29, "Type Safety"),
    ("E", 30, 9999, "Type System"),
    ("W", 0, 9999, "Warnings"),
)


def clean(text: str) -> str:
    return re.sub(r"\s+", " ", text.strip().rstrip(".").strip())


def sort_key(code: str) -> tuple[int, int]:
    # E before W, then numeric.
    return (0 if code[4] == "E" else 1, int(code[5:]))


def group_for(code: str) -> str:
    kind, num = code[4], int(code[5:])
    for gk, lo, hi, label in GROUPS:
        if gk == kind and lo <= num <= hi:
            return label
    return "Type System"


def inline_html(text: str) -> str:
    """Render a rustdoc line as safe inline HTML: intra-doc links unwrapped,
    `code` spans and *emphasis* preserved."""
    text = re.sub(r"\[`?([^`\]]+)`?\]", r"\1", text)  # [`Foo`] / [BSK-X] -> Foo
    text = html.escape(text)
    text = re.sub(r"`([^`]+)`", r"<code>\1</code>", text)
    text = re.sub(r"(?<!\*)\*([^*]+)\*(?!\*)", r"<em>\1</em>", text)
    return text


ENDS_SENTENCE = (".", "!", ")", ":")
FENCE = re.compile(r"^```(\w*)\s*$")


def is_text_line(line: str) -> bool:
    return line != "" and not FENCE.match(line) and not SPEC_REF.match(line)


def parse_body(doc_lines: list[str]) -> list[dict]:
    """Turn the doc-comment lines beneath a header into typed blocks: text
    paragraphs (safe inline HTML) and fenced code blocks (raw, escaped by the
    template). The spec-reference line is dropped."""
    blocks: list[dict] = []
    paragraph: list[str] = []
    code: list[str] | None = None
    lang = "python"

    def flush_paragraph() -> None:
        nonlocal paragraph
        if paragraph:
            blocks.append({"type": "text", "html": inline_html(" ".join(paragraph))})
            paragraph = []

    for line in doc_lines:
        fence = FENCE.match(line)
        if code is not None:
            if fence:
                blocks.append({"type": "code", "lang": lang, "code": "\n".join(code)})
                code = None
            else:
                code.append(line)
            continue
        if fence:
            flush_paragraph()
            code = []
            lang = fence.group(1) or "text"
            continue
        if line == "":
            flush_paragraph()
            continue
        if SPEC_REF.match(line):
            continue
        paragraph.append(line)
    flush_paragraph()
    if code:  # unterminated fence — keep the content rather than drop it
        blocks.append({"type": "code", "lang": lang, "code": "\n".join(code)})
    return blocks


def extract() -> list[dict]:
    """One record per code: summary, body blocks, severity, group, docsUrl."""
    records: dict[str, dict] = {}
    for path in sorted(RULES_DIR.rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        lines = text.splitlines()
        file_docs_url = DOCS_URL.search(text)
        for i, line in enumerate(lines):
            m = HEADER.match(line.strip())
            if not m:
                continue
            code, summary = m.group(1), m.group(2)
            if code in records:
                continue
            # The contiguous //! doc lines following the header line.
            body_lines: list[str] = []
            for follow in lines[i + 1 :]:
                doc = DOC.match(follow.strip())
                if doc is None:
                    break
                body_lines.append(doc.group(1))
            # Stitch a summary that wrapped onto following doc lines (it ends
            # without sentence-final punctuation) before they become body.
            while (
                not summary.rstrip().endswith(ENDS_SENTENCE)
                and body_lines
                and is_text_line(body_lines[0])
            ):
                summary = f"{summary} {body_lines.pop(0)}"
            records[code] = {
                "code": code,
                "severity": "error" if code[4] == "E" else "warning",
                "summary": clean(summary),
                "summaryHtml": inline_html(clean(summary)),
                "body": parse_body(body_lines),
                "group": group_for(code),
                "docsUrl": file_docs_url.group(1)
                if file_docs_url
                else f"{ERRORS_BASE_URL}/{code}",
            }
    return [records[c] for c in sorted(records, key=sort_key)]


def to_markdown(records: list[dict]) -> str:
    rows = ["| Code | Description |", "|---|---|"]
    for r in records:
        rows.append(f"| `{r['code']}` | {r['summary']} |")
    return "\n".join(rows)


def main() -> int:
    records = extract()
    if "--json" in sys.argv:
        print(json.dumps({r["code"]: r["summary"] for r in records}, indent=2))
        return 0
    if "--data" in sys.argv:
        idx = sys.argv.index("--data")
        out = Path(sys.argv[idx + 1]) if idx + 1 < len(sys.argv) else DEFAULT_DATA_OUT
        out.write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
        errors = sum(r["severity"] == "error" for r in records)
        warnings = len(records) - errors
        print(f"Wrote {len(records)} codes ({errors} errors, {warnings} warnings) -> {out}")
        return 0
    if "--check" in sys.argv:
        target = Path(sys.argv[sys.argv.index("--check") + 1]).read_text(encoding="utf-8")
        missing = [r["code"] for r in records if r["code"] not in target]
        if missing:
            print(f"MISSING {len(missing)} codes: {', '.join(missing)}")
            return 1
        print(f"OK: all {len(records)} codes present")
        return 0
    print(to_markdown(records))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
