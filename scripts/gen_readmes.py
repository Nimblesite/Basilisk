#!/usr/bin/env python3
# Implements [README]. See docs/specs/DOCS-README-SPEC.md
"""Render every published README from the single authored source.

Basilisk's front page is published to three storefronts — GitHub, the VS Code
Marketplace / Open VSX (one VSIX, one file), and PyPI. They used to be three
hand-maintained files, so they drifted ([README-PURPOSE]). Now
`docs/readme/README.src.md` (and its Chinese mirror) is the only authored copy,
and every published README is generated from it: identical except for one
paragraph saying which artifact the reader is looking at ([README-IDENTITY]).

Usage:
    python3 scripts/gen_readmes.py            # rewrite the generated READMEs
    python3 scripts/gen_readmes.py --check    # CI: fail if any is stale
"""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE_DIR = ROOT / "docs" / "readme"

REPO_BLOB = "https://github.com/Nimblesite/Basilisk/blob/main"
REPO_RAW = "https://raw.githubusercontent.com/Nimblesite/Basilisk/main"

GENERATED_BANNER = (
    "<!-- GENERATED FILE — DO NOT EDIT.\n"
    "     Source: docs/readme/{source} · Regenerate: python3 scripts/gen_readmes.py\n"
    "     Spec: docs/specs/DOCS-README-SPEC.md [README] -->\n"
)


@dataclass(frozen=True)
class Target:
    """One storefront the README is published to ([README-TARGETS])."""

    key: str
    output: Path
    alt_lang_href: str


@dataclass(frozen=True)
class Source:
    """One authored README and the targets rendered from it ([README-SOURCE])."""

    path: Path
    targets: tuple[Target, ...]


VSIX_README_EN = f"{REPO_BLOB}/vscode-extension/README.md"
VSIX_README_ZH = f"{REPO_BLOB}/vscode-extension/README.zh.md"

SOURCES = (
    Source(
        path=SOURCE_DIR / "README.src.md",
        targets=(
            Target("github", ROOT / "README.md", "README.zh.md"),
            Target("vscode", ROOT / "vscode-extension" / "README.md", VSIX_README_ZH),
            # The wheel listing is English-only; point its switch at the
            # repository's Chinese front page rather than a page PyPI lacks.
            Target("pypi", ROOT / "README-pypi.md", f"{REPO_BLOB}/README.zh.md"),
        ),
    ),
    Source(
        path=SOURCE_DIR / "README.zh.src.md",
        targets=(
            Target("github", ROOT / "README.zh.md", "README.md"),
            Target(
                "vscode", ROOT / "vscode-extension" / "README.zh.md", VSIX_README_EN
            ),
        ),
    ),
)

VARIANT_RE = re.compile(
    r"[ \t]*<!--v:(?P<keys>[a-z,]+)-->\n(?P<body>.*?)[ \t]*<!--/v:(?P=keys)-->\n",
    re.S,
)
# Markdown `](path)` / `](path "title")` and HTML `src="path"` / `href="path"`.
MD_LINK_RE = re.compile(r"\]\((?P<url>[^)\s]+)(?P<rest>[^)]*)\)")
HTML_ATTR_RE = re.compile(r'(?P<attr>\b(?:src|href)=")(?P<url>[^"]+)"')
IMAGE_SUFFIXES = frozenset({".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp"})


class GenerationError(RuntimeError):
    """A source defect that must fail the build rather than ship broken."""


def apply_variants(text: str, key: str) -> str:
    """Keep each `<!--v:…-->` block only for the targets that list it.

    Transform 1 of [README-RENDER].
    """

    def resolve(match: re.Match[str]) -> str:
        keys = match["keys"].split(",")
        return match["body"] if key in keys else ""

    return VARIANT_RE.sub(resolve, text)


def _absolute(url: str) -> str:
    """Absolutise one repo-relative link for a storefront outside the repo."""
    path = url.split("#", 1)[0].rstrip("/")
    if not (ROOT / path).exists():
        raise GenerationError(
            f"relative link `{url}` resolves to no file in the repository — "
            "a published README cannot carry it"
        )
    base = REPO_RAW if Path(path).suffix.lower() in IMAGE_SUFFIXES else REPO_BLOB
    return f"{base}/{url}"


def _is_relative(url: str) -> bool:
    return not url.startswith(("http://", "https://", "#", "mailto:", "//"))


def absolutise_links(text: str) -> str:
    """Rewrite every repo-relative link/image to its canonical GitHub URL.

    Transform 3 of [README-RENDER]. Only the `github` target keeps relative
    links, because only there does the rendered file sit at the repository root.
    """

    def markdown(match: re.Match[str]) -> str:
        url = match["url"]
        if not _is_relative(url):
            return match[0]
        return f"]({_absolute(url)}{match['rest']})"

    def html(match: re.Match[str]) -> str:
        url = match["url"]
        if not _is_relative(url):
            return match[0]
        return f'{match["attr"]}{_absolute(url)}"'

    return HTML_ATTR_RE.sub(html, MD_LINK_RE.sub(markdown, text))


def render(source_text: str, source_name: str, target: Target) -> str:
    """Render one target: variants, tokens, then link absolutisation.

    The three [README-RENDER] transforms, in the order the spec fixes. Token
    substitution is transform 2; `{{altLangHref}}` is a per-target expression of
    one statement, not content ([README-IDENTITY]).
    """
    body = apply_variants(source_text, target.key)
    body = body.replace("{{altLangHref}}", target.alt_lang_href)
    if target.key != "github":
        body = absolutise_links(body)
    return GENERATED_BANNER.format(source=source_name) + body


def strip_authoring_header(text: str) -> str:
    """Drop the source's own `<!-- Implements [README] … -->` preamble."""
    if not text.startswith("<!--"):
        return text
    end = text.index("-->") + len("-->\n")
    return text[end:]


@dataclass(frozen=True)
class Variant:
    """One `<!--v:…-->` block: its body and the targets it renders for."""

    keys: tuple[str, ...]
    body: str

    @property
    def marker(self) -> str:
        """The opening marker, for naming the block in an error message."""
        return f"<!--v:{','.join(self.keys)}-->"

    def is_identity_paragraph(self) -> bool:
        """One blockquote line saying which artifact the reader is looking at.

        The bold opener is load-bearing, not cosmetic: [README-IDENTITY] allows a
        variant block to say *which* artifact this is and nothing else, and every
        identity paragraph in `docs/readme/` opens `> **You are reading …`.
        Accepting any `> ` line would let target-specific prose ride along inside
        a blockquote — exactly the per-target divergence this guard exists to
        stop — so the bold form is required.
        """
        lines = [line for line in self.body.splitlines() if line.strip()]
        return len(lines) == 1 and lines[0].startswith("> **")


def variants(source_text: str) -> tuple[Variant, ...]:
    """Every `<!--v:…-->` block in the source, in document order."""
    return tuple(
        Variant(tuple(match["keys"].split(",")), match["body"])
        for match in VARIANT_RE.finditer(source_text)
    )


def comparable(source_text: str) -> str:
    """The source with every variant block removed.

    What remains is shared by every target verbatim: the language-switch href is
    still its token rather than a per-target URL and links are still relative, so
    this is the text that may not vary between storefronts ([README-IDENTITY]).
    """
    return VARIANT_RE.sub("", source_text).strip()


def _assert_identity_variant(
    variant: Variant, declared: frozenset[str], claimed: frozenset[str], name: str
) -> None:
    """One block must be an identity paragraph for a declared, unclaimed target."""
    if unknown := [key for key in variant.keys if key not in declared]:
        raise GenerationError(
            f"{name}: {variant.marker} renders for no target of this source "
            f"({', '.join(unknown)}) — dead content, see [README-IDENTITY]"
        )
    if not variant.is_identity_paragraph():
        raise GenerationError(
            f"{name}: {variant.marker} is not a single identity paragraph — that "
            "one line is all a target may vary by, see [README-IDENTITY]"
        )
    if repeated := [key for key in variant.keys if key in claimed]:
        raise GenerationError(
            f"{name}: target `{repeated[0]}` carries a second variant block — one "
            "identity paragraph per target, see [README-IDENTITY]"
        )


def assert_only_identity_differs(
    source_text: str, keys: tuple[str, ...], source_name: str
) -> None:
    """[README-IDENTITY]: targets may differ by the identity paragraph alone.

    A `<!--v:…-->` block is the only thing that can make content target-specific,
    so the rule is enforced on the blocks themselves rather than on a text diff
    that has to guess which lines are the identity: everything outside them is
    shared verbatim, and every block is one identity paragraph claimed by exactly
    one declared target.
    """
    if not comparable(source_text):
        raise GenerationError(f"{source_name}: rendered nothing")
    declared = frozenset(keys)
    claimed: set[str] = set()
    for variant in variants(source_text):
        _assert_identity_variant(variant, declared, frozenset(claimed), source_name)
        claimed.update(variant.keys)
    if missing := sorted(declared - claimed):
        raise GenerationError(
            f"{source_name}: target(s) {', '.join(missing)} carry no identity "
            "paragraph — every storefront must say which artifact it is, see "
            "[README-IDENTITY]"
        )


def outputs() -> list[tuple[Path, str]]:
    """Every generated README path with its rendered content."""
    results: list[tuple[Path, str]] = []
    for source in SOURCES:
        raw = strip_authoring_header(source.path.read_text(encoding="utf-8"))
        assert_only_identity_differs(
            raw,
            tuple(target.key for target in source.targets),
            source.path.name,
        )
        results.extend(
            (target.output, render(raw, source.path.name, target))
            for target in source.targets
        )
    return results


def main(argv: list[str]) -> int:
    """Write every generated README, or with `--check` enforce [README-DRIFT]."""
    check = "--check" in argv[1:]
    try:
        generated = outputs()
    except GenerationError as error:
        print(f"gen_readmes: {error}", file=sys.stderr)
        return 2

    stale: list[Path] = []
    for path, content in generated:
        current = path.read_text(encoding="utf-8") if path.exists() else None
        if current == content:
            continue
        if check:
            stale.append(path)
        else:
            path.write_text(content, encoding="utf-8")
            print(f"wrote {path.relative_to(ROOT)}")

    if stale:
        listing = ", ".join(str(path.relative_to(ROOT)) for path in stale)
        print(
            f"gen_readmes: stale generated README(s): {listing}\n"
            "  Edit docs/readme/README.src.md (or its .zh source), then run:\n"
            "    python3 scripts/gen_readmes.py",
            file=sys.stderr,
        )
        return 1
    if check:
        print("READMEs are in sync with docs/readme/")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
