#!/usr/bin/env python3
"""The published READMEs carry the statement and nothing it forbids.

Implements [WITHDRAWAL-SURFACES]. Every storefront front page — GitHub, the VSIX
on Marketplace and Open VSX, PyPI, Zed, Neovim — is generated from
docs/readme/README.src.md with the statement substituted from the messaging spec
([WITHDRAWAL-COPY]). `gen_readmes.py --check` proves they match their source;
these tests prove the source still says the right thing, and that no hand-authored
part of it reintroduces something [WITHDRAWAL-PROHIBITED] bars.

    python3 -m pytest scripts/test_published_readmes.py
"""

from __future__ import annotations

import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from gen_readmes import SOURCES  # noqa: E402
from gen_withdrawal_copy import copy_blocks  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[1]

PUBLISHED = tuple(target.output for source in SOURCES for target in source.targets)

APOLOGY = "https://www.christianfindlay.com/blog/basilisk-conformance-apology"

# Each pattern is something a front page must never say again. Anchored on the
# rendered markdown, so a link, a badge, or a code fence all count.
FORBIDDEN = (
    ("a conformance or pass-rate figure", re.compile(r"\d+(\.\d+)?\s*%")),
    ("install instructions", re.compile(r"\b(pip|pipx|uv tool|brew|scoop|npm)\s+install\b", re.I)),
    ("an editor install link", re.compile(r"vscode:extension", re.I)),
    ("a marketplace or package listing link", re.compile(r"marketplace\.visualstudio\.com|open-vsx\.org|pypi\.org", re.I)),
    ("a competitor comparison", re.compile(r"\b(pyright|mypy|pyrefly|zuban|pylance)\b", re.I)),
    ("a benchmark claim", re.compile(r"\bbenchmark|\bfastest\b", re.I)),
    ("a rule catalogue", re.compile(r"\bBSK-\d{4}\b")),
    ("a `basilisk` invocation", re.compile(r"\bbasilisk (check|analyze|fix|lsp)\b")),
)


class PublishedReadmes(unittest.TestCase):
    """Every storefront front page, as it will be published."""

    def setUp(self) -> None:
        self.readmes = {path: path.read_text(encoding="utf-8") for path in PUBLISHED}
        self.assertTrue(self.readmes, "no published README targets are declared")

    def test_every_readme_opens_with_the_statement(self) -> None:
        copy = copy_blocks()
        for path, text in self.readmes.items():
            with self.subTest(readme=path.relative_to(REPO_ROOT)):
                self.assertIn(f"# {copy.title}", text)
                for paragraph in copy.full:
                    self.assertIn(paragraph, text)

    def test_every_readme_tells_the_reader_what_to_do(self) -> None:
        # The action block is the only part that asks something of the reader,
        # so it is the part most likely to be trimmed for length.
        copy = copy_blocks()
        for path, text in self.readmes.items():
            with self.subTest(readme=path.relative_to(REPO_ROOT)):
                for paragraph in copy.action:
                    self.assertIn(paragraph, text)
                self.assertIn("Remove Basilisk from your pipeline", text)

    def test_every_readme_links_the_apology_without_quoting_it(self) -> None:
        for path, text in self.readmes.items():
            with self.subTest(readme=path.relative_to(REPO_ROOT)):
                self.assertIn(APOLOGY, text)
                self.assertNotRegex(text, r"I (was|am) (wrong|sorry)|in my own words")

    def test_no_readme_says_anything_prohibited(self) -> None:
        for path, text in self.readmes.items():
            for label, pattern in FORBIDDEN:
                with self.subTest(readme=path.relative_to(REPO_ROOT), forbidden=label):
                    self.assertIsNone(
                        pattern.search(text),
                        f"{path.relative_to(REPO_ROOT)} contains {label}",
                    )

    def test_no_readme_shows_a_product_image(self) -> None:
        # Screenshots are release evidence for a product that is being delisted;
        # a marketing image beside a withdrawal notice reads as still selling.
        for path, text in self.readmes.items():
            with self.subTest(readme=path.relative_to(REPO_ROOT)):
                self.assertNotRegex(text, r"!\[[^\]]*\]\(|<img\b")


if __name__ == "__main__":
    unittest.main()
