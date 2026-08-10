#!/usr/bin/env python3
"""The public-copy scan catches what it claims to, and covers every storefront.

Implements [WITHDRAWAL-SURFACES]. A scan that matches nothing passes silently
and proves nothing, so each rule is exercised against text it must reject and
against the approved copy it must accept.

    python3 -m pytest scripts/test_check_public_copy.py
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from check_public_copy import REPO_ROOT, RULES, scan, surfaces  # noqa: E402

# One example per rule id: text that surface must never carry again.
OFFENDING = {
    "measured-figure": "Basilisk passed 99.7% of the suite.",
    "conformance-score": "Its conformance score was 412 of 500.",
    "install-instruction": "Get started: `pip install basilisk-python`.",
    "rule-count": "Ships with 340 rules covering the typing specification.",
    "feature-marketing": "A strict-by-default Python type checker.",
    "scoping-reassurance": "Only a few rules are affected; the rest is fine.",
    "shipping-claim": "## Status\n\nWorking — powers the editor integration.",
    "quoted-apology": (
        "> I got this wrong.\n"
        "> https://www.christianfindlay.com/blog/basilisk-conformance-apology"
    ),
}


class Rules(unittest.TestCase):
    def test_every_rule_has_an_example_and_rejects_it(self):
        self.assertEqual(
            sorted(OFFENDING), sorted(rule.id for rule in RULES), "rules and examples"
        )
        for rule in RULES:
            with self.subTest(rule=rule.id):
                self.assertRegex(OFFENDING[rule.id], rule.pattern)

    def test_no_rule_fires_on_a_different_rules_example(self):
        # Overlapping patterns would make a failure report point at the wrong
        # prohibition, which is worse than not catching it at all.
        for rule in RULES:
            for other_id, text in OFFENDING.items():
                if other_id == rule.id:
                    continue
                with self.subTest(rule=rule.id, text=other_id):
                    self.assertIsNone(rule.pattern.search(text))

    def test_the_approved_copy_trips_nothing(self):
        # The statement names the python/typing conformance results and links
        # PR #2330. Both must survive the scan: a rule that cannot tell a
        # source link from a score would force the copy to be watered down.
        readme = REPO_ROOT / "README.md"
        self.assertEqual(scan(readme), [])
        self.assertIn("conformance results", readme.read_text(encoding="utf-8"))


class Coverage(unittest.TestCase):
    def test_every_storefront_is_scanned(self):
        scanned = {path.relative_to(REPO_ROOT).as_posix() for path in surfaces()}
        for required in (
            "README.md",
            "README-pypi.md",
            "vscode-extension/README.md",
            "vscode-extension/package.json",
            "basilisk-zed/README.md",
            "basilisk-zed/extension.toml",
            "basilisk.nvim/README.md",
            "pyproject.toml",
            ".github/release-templates/basilisk.rb.tmpl",
            ".github/release-templates/basilisk.json.tmpl",
        ):
            with self.subTest(surface=required):
                self.assertIn(required, scanned)

    def test_every_crate_readme_is_scanned(self):
        scanned = {path.relative_to(REPO_ROOT).as_posix() for path in surfaces()}
        on_disk = {
            path.relative_to(REPO_ROOT).as_posix()
            for path in (REPO_ROOT / "crates").glob("*/README.md")
        }
        self.assertTrue(on_disk)
        self.assertTrue(on_disk <= scanned, on_disk - scanned)

    def test_the_repository_is_clean(self):
        offenders = {
            path.relative_to(REPO_ROOT).as_posix(): scan(path)
            for path in surfaces()
            if scan(path)
        }
        self.assertEqual(offenders, {})


if __name__ == "__main__":
    unittest.main()
