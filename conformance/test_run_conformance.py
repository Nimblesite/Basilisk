"""Regression tests for the conformance runner's environment bootstrap."""

from __future__ import annotations

import tempfile
import unittest
import subprocess
import sys
from pathlib import Path

from run_conformance import _is_current_venv


ROOT = Path(__file__).resolve().parents[1]


class HarnessEnvironmentTests(unittest.TestCase):
    def test_symlinked_interpreter_does_not_impersonate_virtualenv(self) -> None:
        """A shared executable target must not be used as venv identity."""
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            system_prefix = root / "system"
            venv = root / "venv"
            system_prefix.mkdir()
            venv.mkdir()

            self.assertFalse(_is_current_venv(venv, prefix=str(system_prefix)))

    def test_matching_prefix_identifies_current_virtualenv(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            venv = Path(temp) / "venv"
            venv.mkdir()

            self.assertTrue(_is_current_venv(venv, prefix=str(venv)))


class GeneratedReferenceTests(unittest.TestCase):
    def test_checked_in_conformance_references_match_the_live_report(self) -> None:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "gen_conformance_reference.py"),
                "--check",
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )

        self.assertEqual(
            result.returncode,
            0,
            result.stdout + result.stderr,
        )


if __name__ == "__main__":
    unittest.main()
