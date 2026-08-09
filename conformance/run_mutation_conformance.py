#!/usr/bin/env python3
# Implements [CHKARCH-CONFORMANCE-MUTATION]. See docs/CONFORMANCE-INTEGRITY-AUDIT.md
"""Run the AST-PRESERVING MUTATED python/typing fixture regression INDICATOR.

Why this exists: the 2026-08 integrity audit (docs/CONFORMANCE-INTEGRITY-AUDIT.md)
found checker code fitted to the conformance fixtures' exact spellings. A
checker that reasons structurally must produce IDENTICAL verdicts when a
fixture consistently renames its typing imports (``ClassVar as AuditClassVar``)
or reformats whitespace — the mutations in
``conformance/mutate_typing_conformance.py`` (sharkdp's harness, vendored
verbatim from https://gist.github.com/sharkdp/3f3266fd9c67d22137e2b6c015c5f206)
change neither semantics, line numbers, nor expected-error markers.

The flow mirrors ``run_conformance.py`` and reimplements nothing:

  1. clone ``python/typing`` fresh at the last revision carrying the Basilisk
     adapter (``a4906624f170c169cf667f962080c56d5a5ba6ff`` by default),
  2. apply the vendored mutation script to the clone's ``conformance/tests``,
  3. run the suite's OWN unmodified ``conformance/src/main.py --only-run
     basilisk`` against the freshly built release binary via ``BASILISK_BIN``,
  4. count files whose harness-computed ``conformance_automated`` is ``Pass``.

The score GATES NOTHING. It is printed to be read, and this script exits 0
whatever it measures. It used to be a ratchet with a floor in
``conformance/mutation_conformance_baseline.json`` that failed the build on a
drop; that was deleted, because CLAUDE.md forbids a conformance gate,
threshold, or ratchet anywhere, and because a floor rewards keeping the number
up rather than making the analysis right. A drop caused by REMOVING
text-matched logic is progress and must be recorded as such.

What the number means: a checker that reasons structurally produces identical
verdicts on the mutated suite and the pristine one, so a GAP between the two
rates locates spelling dependence. The gap is the signal — not the height of
either rate.

Like the pristine-fixture result, this is internal regression evidence. The
adapter has been removed from ``python/typing@main``, and neither this rate nor
the pristine rate is a current official conformance score.

Usage:
    python3 conformance/run_mutation_conformance.py --bin PATH [--ref REF]
        [--suite-dir DIR] [--reuse-clone]
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
# NO BASELINE PATH. The ratchet that read one was deleted; see the banner in
# `main`. Do not reintroduce a floor file here or anywhere else.
MUTATOR_PATH = REPO_ROOT / "conformance" / "mutate_typing_conformance.py"


def clone_suite(ref: str, suite_dir: Path, reuse: bool) -> Path:
    """Clone python/typing at ``ref`` into ``suite_dir`` (fresh by default)."""
    if suite_dir.exists():
        if not reuse:
            shutil.rmtree(suite_dir)
    if not suite_dir.exists():
        subprocess.run(
            [
                "git",
                "clone",
                "--quiet",
                "https://github.com/python/typing",
                str(suite_dir),
            ],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(suite_dir), "checkout", "--quiet", ref],
            check=True,
        )
    return suite_dir


def apply_mutations(suite_dir: Path) -> None:
    """Run the vendored mutation harness over the clone, verbatim."""
    result = subprocess.run(
        [sys.executable, str(MUTATOR_PATH), str(suite_dir)],
        check=True,
        capture_output=True,
        text=True,
    )
    summary = result.stdout.strip().splitlines()[-1:] or ["(no output)"]
    print(f"mutation harness: {summary[0]}")


def run_harness(suite_dir: Path, binary: Path) -> Path:
    """Run the suite's own scorer against ``binary``; return the results dir."""
    conformance_dir = suite_dir / "conformance"
    env = {
        "BASILISK_BIN": str(binary.resolve()),
        "PATH": "/usr/bin:/bin:/usr/local/bin",
    }
    import os

    merged_env = {**os.environ, **env}
    subprocess.run(
        [sys.executable, "src/main.py", "--only-run", "basilisk"],
        cwd=conformance_dir,
        env=merged_env,
        check=True,
        capture_output=True,
        text=True,
    )
    return conformance_dir / "results" / "basilisk"


def score(results_dir: Path) -> tuple[int, int, list[str]]:
    """Count harness-computed passes; the verdicts are the harness's own."""
    passed = total = 0
    failures: list[str] = []
    for toml_path in sorted(results_dir.glob("*.toml")):
        if toml_path.stem == "version":
            continue
        data = tomllib.loads(toml_path.read_text())
        total += 1
        if data.get("conformance_automated") == "Pass":
            passed += 1
        else:
            failures.append(toml_path.stem)
    return passed, total, failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--bin", required=True, type=Path, help="basilisk release binary"
    )
    # ⚠️ python/typing@main removed the Basilisk adapter in c43d32e (typing#2330),
    # so `--only-run basilisk` is invalid at HEAD until it is restored. The full
    # SHA below is the last commit whose own unmodified harness carries
    # BasiliskTypeChecker; move this default back to "main" only when the
    # adapter returns upstream.
    parser.add_argument(
        "--ref",
        default="a4906624f170c169cf667f962080c56d5a5ba6ff",
        help="python/typing fixture ref to run (see adapter note above)",
    )
    parser.add_argument(
        "--suite-dir",
        type=Path,
        default=REPO_ROOT / "target" / "typing-mutated",
        help="where to clone the suite",
    )
    parser.add_argument("--reuse-clone", action="store_true")
    args = parser.parse_args()

    suite_dir = clone_suite(args.ref, args.suite_dir, args.reuse_clone)
    apply_mutations(suite_dir)
    results_dir = run_harness(suite_dir, args.bin)
    passed, total, failures = score(results_dir)
    rate = passed / total if total else 0.0
    print(f"MUTATED conformance: {passed}/{total} ({rate:.1%})")
    if failures:
        print("failing files:", ", ".join(failures))

    # ###################################################################
    # # DELETED — the ratchet. DO NOT RESTORE IT IN ANY FORM.
    # #
    # # This read `min_pass_rate` from a baseline file and returned 1 when
    # # the measured rate fell below it, making the mutated conformance
    # # score a GATE. CLAUDE.md forbids that outright: "Never a gate,
    # # threshold, or ratchet anywhere ... Do not reintroduce it in that
    # # file, `make test`, CI, or a script." The floor was the incentive
    # # that produced the fitting this whole effort exists to undo, and a
    # # floor on the MUTATED suite is the same incentive wearing a better
    # # hat — it rewards keeping a number up rather than making the
    # # analysis right.
    # #
    # # A drop caused by removing text-matched logic is PROGRESS. The
    # # number above is printed to be READ. `--update-baseline` is gone
    # # with the floor it wrote.
    # ###################################################################
    return 0


if __name__ == "__main__":
    sys.exit(main())
