#!/usr/bin/env python3
"""Regression tests for the book's durable editorial contract."""

from __future__ import annotations

import hashlib
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
        editorial_brief = (BOOK_ROOT / "EDITORIAL-BRIEF.md").read_text(encoding="utf-8")
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

    def test_ready_screenshots_are_direct_captures_of_the_pinned_release(self) -> None:
        """A UI-shaped diagram cannot pass as screenshot evidence."""
        book = json.loads((BOOK_ROOT / "book.json").read_text(encoding="utf-8"))
        ledger = json.loads((BOOK_ROOT / "figures.json").read_text(encoding="utf-8"))
        screenshots = [
            figure
            for figure in ledger["figures"]
            if figure["kind"] in {"screenshot", "annotated-screenshot"}
            and figure["status"] == "ready"
        ]

        self.assertGreaterEqual(len(screenshots), 2)
        self.assertTrue(
            {"shot-09-config-editor", "shot-09-config-preview"}
            <= {figure["id"] for figure in screenshots}
        )
        for figure in screenshots:
            capture = figure["capture"]
            raw_master = BOOK_ROOT / capture["rawMaster"]
            with raw_master.open("rb") as source:
                digest = hashlib.file_digest(source, "sha256").hexdigest()
            self.assertEqual(capture["authenticity"], "direct-release-capture")
            self.assertEqual(capture["basiliskVersion"], book["basiliskRelease"])
            self.assertEqual(capture["releaseTag"], book["basiliskReleaseTag"])
            self.assertEqual(capture["releaseCommit"], book["basiliskReleaseCommit"])
            self.assertEqual(capture["masterSha256"], digest)
            self.assertEqual(figure["master"], capture["rawMaster"])
            self.assertTrue(figure["path"].startswith("assets/screenshots/"))

    def test_book_instructions_forbid_relabelled_fake_screenshots(self) -> None:
        """Keep the no-mock rule in both agent and author instructions."""
        repository_instructions = (REPOSITORY_ROOT / "CLAUDE.md").read_text(
            encoding="utf-8"
        )
        book_instructions = (BOOK_ROOT / "README.md").read_text(encoding="utf-8")
        visual_contract = (BOOK_ROOT / "VISUAL-DESIGN-SYSTEM.md").read_text(
            encoding="utf-8"
        )

        self.assertIn(
            "NEVER mock, redraw, reconstruct, generate", repository_instructions
        )
        self.assertIn("even if it is labelled a diagram", book_instructions)
        self.assertIn("hand-built reconstruction is a fake screenshot", visual_contract)


if __name__ == "__main__":
    unittest.main()
