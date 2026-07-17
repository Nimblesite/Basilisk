#!/usr/bin/env python3
"""Regression tests for the book's durable editorial contract."""

from __future__ import annotations

import json
import unittest
from pathlib import Path


BOOK_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = BOOK_ROOT.parent


class BookContractTests(unittest.TestCase):
    """Keep product policy out of edition-specific assumptions."""

    def test_python_typing_contract_is_pep_first_not_version_pinned(self) -> None:
        """The book must not declare one Python release as canonical."""
        book = json.loads((BOOK_ROOT / "book.json").read_text(encoding="utf-8"))
        metadata = (BOOK_ROOT / "metadata.yaml").read_text(encoding="utf-8")
        editorial_brief = (BOOK_ROOT / "EDITORIAL-BRIEF.md").read_text(
            encoding="utf-8"
        )
        outline = (BOOK_ROOT / "OUTLINE.md").read_text(encoding="utf-8")
        repository_instructions = (REPOSITORY_ROOT / "CLAUDE.md").read_text(
            encoding="utf-8"
        )

        self.assertNotIn("canonicalPythonVersion", book)
        self.assertNotIn('python-version: "3.12"', metadata)
        self.assertNotIn("Canonical language target: Python 3.12", editorial_brief)
        self.assertNotIn(
            "Python 3.12 as the canonical language target for this edition",
            outline,
        )
        self.assertNotIn("canonical Python **3.12**", repository_instructions)


if __name__ == "__main__":
    unittest.main()
