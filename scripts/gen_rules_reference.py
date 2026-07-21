#!/usr/bin/env python3
"""Generate the canonical diagnostic-code reference from the checker source.

Single source of truth: the diagnostic-code header — and the doc-comment body
beneath it — on each rule module under crates/basilisk-checker/src/rules/. A
header is either an opt-in `//! BSK-E####: <description>` (`E`, `W`, or `I`) code
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
from csv import DictReader
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RULES_DIR = ROOT / "crates" / "basilisk-checker" / "src" / "rules"
DEFAULT_DATA_OUT = ROOT / "website" / "src" / "_data" / "rules.json"
CONFORMANCE_STATUS = ROOT / "conformance" / "conformance_status.csv"
ERRORS_BASE_URL = "https://www.basilisk-python.dev/errors"
TYPING_SPEC_BASE_URL = "https://typing.python.org/en/latest/spec"

# [WEBSITE-ERROR-PAGES-REFERENCES]: canonical documentation for every code.
# Each code-name prefix maps to its chapter of the maintained typing spec
# (https://typing.python.org/en/latest/spec/ — titles and filenames taken from
# that index verbatim). Conformance categories are named after these chapters
# upstream; the trailing entries cover Basilisk's general soundness rules whose
# prefix is not a conformance category.
SPEC_CHAPTER_BY_PREFIX = {
    "aliases": ("Type aliases", "aliases.html"),
    "annotations": ("Type annotations", "annotations.html"),
    "callables": ("Callables", "callables.html"),
    "classes": ("Class type assignability", "class-compat.html"),
    "constructors": ("Constructors", "constructors.html"),
    "dataclasses": ("Dataclasses", "dataclasses.html"),
    "directives": ("Type checker directives", "directives.html"),
    "enums": ("Enumerations", "enums.html"),
    "exceptions": ("Exceptions", "exceptions.html"),
    "generics": ("Generics", "generics.html"),
    "historical": ("Historical and deprecated features", "historical.html"),
    "literals": ("Literals", "literal.html"),
    "namedtuples": ("Named Tuples", "namedtuples.html"),
    "narrowing": ("Type narrowing", "narrowing.html"),
    "overloads": ("Overloads", "overload.html"),
    "protocols": ("Protocols", "protocol.html"),
    "qualifiers": ("Type qualifiers", "qualifiers.html"),
    "specialtypes": ("Special types in annotations", "special-types.html"),
    "tuples": ("Tuples", "tuples.html"),
    "typeddicts": ("Typed dictionaries", "typeddict.html"),
    "typeforms": ("Type forms", "type-forms.html"),
    "assignment": ("Type system concepts", "concepts.html"),
    "calls": ("Callables", "callables.html"),
    "dict": ("Type system concepts", "concepts.html"),
    "imports": ("Distributing type information", "distributing.html"),
    "match": ("Type narrowing", "narrowing.html"),
    "returns": ("Type system concepts", "concepts.html"),
    "version": ("Generics", "generics.html"),
}

# The accepted typing PEPs each spec chapter incorporates — every rule under
# the prefix links these on top of any PEP its own doc comment cites. Numbers
# only; labels stay "PEP NNN" so nothing here can drift from peps.python.org.
PEPS_BY_PREFIX = {
    "aliases": (484, 613, 695),
    "annotations": (3107, 484, 526),
    "callables": (484, 612, 692),
    "classes": (484, 526, 698),
    "constructors": (484,),
    "dataclasses": (557, 681),
    "directives": (484, 702),
    "enums": (435,),
    "generics": (484, 612, 646, 673, 695, 696),
    "historical": (484,),
    "literals": (586, 675),
    "namedtuples": (484,),
    "narrowing": (647, 742),
    "overloads": (484,),
    "protocols": (544,),
    "qualifiers": (526, 591, 593),
    "specialtypes": (484,),
    "tuples": (484, 646),
    "typeddicts": (589, 655, 705, 728),
    "typeforms": (747,),
    "assignment": (484,),
    "calls": (484,),
    "imports": (561,),
    "match": (634,),
    "returns": (484,),
    "packaging": (621,),
}

# Rules governed by something other than the typing spec link that authority
# instead: the Python language reference, or a tool's own documentation.
LANGUAGE_REFS_BY_PREFIX = {
    "names": (
        {
            "label": "Python language reference: Naming and binding",
            "url": "https://docs.python.org/3/reference/executionmodel.html#naming-and-binding",
        },
    ),
    "uv": (
        {
            "label": "uv: Locking and syncing",
            "url": "https://docs.astral.sh/uv/concepts/projects/sync/",
        },
    ),
}

# House rules (BSK codes) carry no conformance-category prefix; each maps to
# the chapter/PEPs documenting the mechanism it polices. The suppression rules
# (BSK-0060..0063) police Basilisk's own directives — no upstream doc exists —
# and BSK-0025's doc comment already cites PEP 698 directly.
REFERENCE_PREFIX_BY_BSK_CODE = {
    "BSK-0001": "annotations",
    "BSK-0002": "annotations",
    "BSK-0003": "annotations",
    "BSK-0004": "annotations",
    "BSK-0005": "annotations",
    "BSK-0011": "packaging",
    "BSK-0012": "packaging",
    "BSK-0013": "uv",
    "BSK-0014": "specialtypes",
    "BSK-0040": "annotations",
    "BSK-0050": "annotations",
    "BSK-0152": "imports",
}

HEADER = re.compile(r"//!\s*(BSK-\d{4}|`[a-z0-9_]+`):\s*(.*)")
DOC = re.compile(r"//!\s?(.*)")
PEP_MENTION = re.compile(r"\bPEP (\d{1,4})\b")
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

# Coarse groups for filtering/badging on the website, derived from the rule's
# own tags — codes carry no severity class ([CHKARCH-DIAG-CODES]).
GROUP_BY_TAG = {
    "strictness": "Missing Annotations",
    "style": "Style",
    "redundancy": "Redundancy",
    "suppressions": "Suppressions",
    "dependencies": "Dependencies",
    "imports": "Imports",
    "stubs": "Stubs",
}


def pep_categories() -> frozenset[str]:
    """Read the canonical python/typing category vocabulary used by Basilisk.

    The checker validates the same CSV-backed vocabulary in [CHKTAG-TESTS].
    Reading it here keeps the website consumer on that source instead of
    maintaining a parallel category list.
    """
    with CONFORMANCE_STATUS.open(encoding="utf-8", newline="") as handle:
        return frozenset(
            row["category"] for row in DictReader(handle) if row.get("category")
        )


PEP_CATEGORIES = pep_categories()


def clean(text: str) -> str:
    return re.sub(r"\s+", " ", text.strip().rstrip(".").strip())


def is_bsk(code: str) -> bool:
    return code.startswith("BSK-")


def scope_for(provenance: str) -> str:
    # The command partition [CHKARCH-COMMANDS]: pep-tagged rules belong to
    # `basilisk check` (always run); everything else to `basilisk analyze`.
    return "check" if provenance == "pep" else "analyze"


def sort_key(code: str) -> tuple[int, int, str]:
    # BSK codes first (numeric), then named conformance codes alphabetically.
    if is_bsk(code):
        return (0, int(code[4:]), "")
    return (1, 0, code)


def group_for(code: str, free_form_tags: list[str]) -> str:
    if not is_bsk(code):
        # Named conformance rules span the broad type-system surface.
        return "Type System"
    for tag in free_form_tags:
        if tag in GROUP_BY_TAG:
            return GROUP_BY_TAG[tag]
    return "Type System"


def pep_url(number: int) -> str:
    return f"https://peps.python.org/pep-{number:04d}/"


def link_peps(text: str) -> str:
    """Turn every `PEP NNN` mention into a link to its canonical page."""
    return PEP_MENTION.sub(
        lambda m: f'<a href="{pep_url(int(m.group(1)))}">PEP {int(m.group(1))}</a>',
        text,
    )


def inline_html(text: str) -> str:
    """Render a rustdoc line as safe inline HTML: intra-doc links unwrapped,
    `code` spans and *emphasis* preserved, PEP mentions linked
    ([WEBSITE-ERROR-PAGES-REFERENCES])."""
    text = re.sub(r"\[`?([^`\]]+)`?\]", r"\1", text)  # [`Foo`] / [BSK-X] -> Foo
    text = html.escape(text)
    text = re.sub(r"`([^`]+)`", r"<code>\1</code>", text)
    text = re.sub(r"(?<!\*)\*([^*]+)\*(?!\*)", r"<em>\1</em>", text)
    return link_peps(text)


# Implements [WEBSITE-ERROR-PAGES-REFERENCES]: the canonical-documentation list
# for one code — its typing-spec chapter, then the chapter's PEPs merged with
# every PEP the rule's own doc comment cites, then any language-reference link.
def references_for(code: str, doc_text: str) -> list[dict]:
    prefix = REFERENCE_PREFIX_BY_BSK_CODE.get(code, code.partition("_")[0])
    refs: list[dict] = []
    chapter = SPEC_CHAPTER_BY_PREFIX.get(prefix)
    if chapter:
        title, page = chapter
        refs.append(
            {
                "label": f"Typing spec: {title}",
                "url": f"{TYPING_SPEC_BASE_URL}/{page}",
            }
        )
    mentioned = {int(n) for n in PEP_MENTION.findall(doc_text)}
    for number in sorted(mentioned.union(PEPS_BY_PREFIX.get(prefix, ()))):
        refs.append({"label": f"PEP {number}", "url": pep_url(number)})
    refs.extend(LANGUAGE_REFS_BY_PREFIX.get(prefix, ()))
    return refs


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
    """One record per code, including its canonical checker tag set."""
    records: dict[str, dict] = {}
    for path in sorted(RULES_DIR.rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        lines = text.splitlines()
        file_docs_url = DOCS_URL.search(text)
        # Provenance and opt-in tags come from the rule's own opt_in_spec, not
        # its cosmetic code prefix. PEP category tags use the same canonical
        # conformance CSV vocabulary validated by rule_tags.rs.
        provenance = "basilisk" if OPT_IN.search(text) else "pep"
        tags_match = OPT_IN_TAGS.search(text)
        free_form_tags = TAG.findall(tags_match.group(1)) if tags_match else []
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
            category = code.partition("_")[0]
            tags = (
                ["basilisk", *free_form_tags]
                if provenance == "basilisk"
                else ["pep", *([category] if category in PEP_CATEGORIES else [])]
            )
            records[code] = {
                "code": code,
                "scope": scope_for(provenance),
                "provenance": provenance,
                "tags": tags,
                "summary": clean(summary),
                "summaryHtml": inline_html(clean(summary)),
                "body": parse_body(body_lines),
                "group": group_for(code, free_form_tags),
                "docsUrl": file_docs_url.group(1)
                if file_docs_url
                else f"{ERRORS_BASE_URL}/{code}",
                "references": references_for(code, " ".join([summary, *body_lines])),
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
        # record per code (summary, body blocks, scope, group, docsUrl).
        idx = sys.argv.index("--data")
        out = Path(sys.argv[idx + 1]) if idx + 1 < len(sys.argv) else DEFAULT_DATA_OUT
        out.write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
        check = sum(r["scope"] == "check" for r in records)
        analyze = len(records) - check
        print(
            f"Wrote {len(records)} codes ({check} check-scope PEP rules, "
            f"{analyze} analyze-scope Basilisk rules) -> {out}"
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
