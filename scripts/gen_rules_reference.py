#!/usr/bin/env python3
"""Generate the canonical diagnostic-code reference from the checker source.

Single source of truth: the diagnostic-code header — and the doc-comment body
beneath it — on each rule module under crates/basilisk-checker/src/rules/. A
header is either an opt-in `//! BSK-E####: <description>` (or `BSK-W####`) code
or a PEP-conformance `//! `code_name`: <description>` code (the conformance
rules are named after their python/typing conformance test, e.g.
``//! `protocols_explicit`: ...``). Both styles are extracted so every code the
CLI can emit gets a page.

Usage:
    python3 scripts/gen_rules_reference.py             # print a Markdown table
    python3 scripts/gen_rules_reference.py --json       # emit code->summary JSON
    python3 scripts/gen_rules_reference.py --data [OUT] # write the rich rules
                                                        # data Eleventy consumes
                                                        # (default: website/src/
                                                        # _data/rules.json)
    python3 scripts/gen_rules_reference.py --check FILE  # verify FILE contains
                                                        # every current code

This is the generator behind [WEBSITE-ERROR-PAGES-PURPOSE]: a landing page for
EVERY diagnostic code, built from the checker source so the pages can never drift
from the diagnostics the binary emits.
The `--data` output ([WEBSITE-ERROR-PAGES-DATA]) drives both the complete
reference table and the per-code /errors/BSK-XXXX/ pages on the website, so the
pages the CLI deep-links to (`see: https://www.basilisk-python.dev/errors/BSK-EXXXX`)
can never drift from the checker. The `--check` mode backs the CI drift guard
([WEBSITE-ERROR-PAGES-DRIFT]). Run it after adding or renaming a rule. See
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

HEADER = re.compile(r"//!\s*(BSK-[EW]\d{4}|`[a-z0-9_]+`):\s*(.*)")
DOC = re.compile(r"//!\s?(.*)")
DOCS_URL = re.compile(r'docs_url:\s*"([^"]+)"')
SPEC_REF = re.compile(r"^Implements ")
# A rule is Basilisk-original (off by default, opt-in only) iff it overrides
# `opt_in_spec` to return `Some(..)`; core PEP-conformance rules leave it `None`.
# This reads the checker's real provenance signal (`Rule::opt_in_spec`, the single
# source of rule provenance per [CHKTAG-PROVENANCE]) — never the cosmetic `BSK-`
# code prefix, which [CHKTAG-BSK-PREFIX] declares semantically meaningless.
# `[^{]*` stops at the body brace, so a `Some(` in another fn can't false-match.
OPT_IN = re.compile(r"fn opt_in_spec\b[^{]*\{\s*Some\(")
# The free-form tags an opt-in rule declares (`tags: &["strictness", ..]`). These
# are the checker's own `OptInSpec.tags` — e.g. `strictness` marks the rules that
# make annotations mandatory beyond the spec. Non-greedy up to the first `tags:`
# inside the single opt_in_spec body; `TAG` pulls each quoted entry out.
OPT_IN_TAGS = re.compile(
    r"fn opt_in_spec\b[^{]*\{\s*Some\([\s\S]*?tags:\s*&\[([^\]]*)\]"
)
TAG = re.compile(r'"([^"]+)"')

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


def is_bsk(code: str) -> bool:
    return code.startswith("BSK-")


def severity_for(code: str) -> str:
    # BSK opt-in codes carry severity in the letter; named PEP-conformance
    # codes are all type errors (spec violations).
    if is_bsk(code):
        return "error" if code[4] == "E" else "warning"
    return "error"


def sort_key(code: str) -> tuple[int, int, int, str]:
    # BSK opt-in codes first (E before W, then numeric), then named
    # conformance codes alphabetically.
    if is_bsk(code):
        return (0, 0 if code[4] == "E" else 1, int(code[5:]), "")
    return (1, 0, 0, code)


def group_for(code: str) -> str:
    if not is_bsk(code):
        # Named conformance rules span the broad type-system surface.
        return "Type System"
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


# Implements [WEBSITE-ERROR-PAGES-PURPOSE]: build one record per diagnostic code
# directly from the checker rule sources, so the generated /errors/<code>/ pages
# can never drift from the diagnostics the binary actually emits.
def extract() -> list[dict]:
    """One record per code: summary, body blocks, severity, group, docsUrl."""
    records: dict[str, dict] = {}
    for path in sorted(RULES_DIR.rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        lines = text.splitlines()
        file_docs_url = DOCS_URL.search(text)
        # Provenance and opt-in tags from the rule's own opt_in_spec, not its
        # code prefix. Core PEP rules carry no opt-in tags (empty list).
        provenance = "basilisk" if OPT_IN.search(text) else "pep"
        tags_match = OPT_IN_TAGS.search(text)
        tags = TAG.findall(tags_match.group(1)) if tags_match else []
        for i, line in enumerate(lines):
            m = HEADER.match(line.strip())
            if not m:
                continue
            code, summary = m.group(1).strip("`"), m.group(2)
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
                "severity": severity_for(code),
                "provenance": provenance,
                "tags": tags,
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
        # [WEBSITE-ERROR-PAGES-DATA]: write website/src/_data/rules.json — one
        # record per code (summary, body blocks, severity, group, docsUrl).
        idx = sys.argv.index("--data")
        out = Path(sys.argv[idx + 1]) if idx + 1 < len(sys.argv) else DEFAULT_DATA_OUT
        out.write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
        errors = sum(r["severity"] == "error" for r in records)
        warnings = len(records) - errors
        opt_in = sum(r["provenance"] == "basilisk" for r in records)
        pep = len(records) - opt_in
        print(
            f"Wrote {len(records)} codes ({errors} errors, {warnings} warnings; "
            f"{pep} PEP-conformance, {opt_in} opt-in) -> {out}"
        )
        return 0
    if "--check" in sys.argv:
        # [WEBSITE-ERROR-PAGES-DRIFT]: assert FILE contains every current code so
        # CI fails when a rule is added/renamed without regenerating rules.json.
        target = Path(sys.argv[sys.argv.index("--check") + 1]).read_text(
            encoding="utf-8"
        )
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
