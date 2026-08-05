#!/usr/bin/env python3
# Implements [CHKARCH-CONFORMANCE-MUTATION]. See docs/CONFORMANCE-INTEGRITY-AUDIT.md
"""Run the AST-PRESERVING MUTATED python/typing fixture regression gate.

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

The score is a RATCHET (``conformance/mutation_conformance_baseline.json``):
the mutated pass rate may only rise. A drop means a change re-introduced
spelling- or formatting-dependent verdicts and FAILS the build. Raising the
floor to a new high-water mark is done by re-running this script with
``--update-baseline`` and committing the result alongside the change that
earned it.

Like the pristine-fixture result, this is internal regression evidence. The
adapter has been removed from ``python/typing@main``, and neither this rate nor
the pristine rate is a current official conformance score.

Usage:
    python3 conformance/run_mutation_conformance.py --bin PATH [--ref REF]
        [--suite-dir DIR] [--reuse-clone] [--update-baseline]
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BASELINE_PATH = REPO_ROOT / "conformance" / "mutation_conformance_baseline.json"
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
    parser.add_argument(
        "--update-baseline",
        action="store_true",
        help="write the measured rate as the new ratchet floor",
    )
    args = parser.parse_args()

    suite_dir = clone_suite(args.ref, args.suite_dir, args.reuse_clone)
    apply_mutations(suite_dir)
    results_dir = run_harness(suite_dir, args.bin)
    passed, total, failures = score(results_dir)
    rate = passed / total if total else 0.0
    print(f"MUTATED conformance: {passed}/{total} ({rate:.1%})")
    if failures:
        print("failing files:", ", ".join(failures))

    baseline = json.loads(BASELINE_PATH.read_text()) if BASELINE_PATH.exists() else {}
    floor = float(baseline.get("min_pass_rate", 0.0))

    if args.update_baseline:
        BASELINE_PATH.write_text(
            json.dumps(
                {
                    "min_pass_rate": round(rate, 4),
                    "measured_passed": passed,
                    "measured_total": total,
                    "note": (
                        "RATCHET — mutated-suite pass rate may only rise. "
                        "See run_mutation_conformance.py."
                    ),
                },
                indent=2,
            )
            + "\n"
        )
        print(f"baseline updated: min_pass_rate={rate:.4f}")
        return 0

    if rate < floor:
        print(
            f"FAIL: mutated pass rate {rate:.1%} fell below the ratchet floor "
            f"{floor:.1%} — a change re-introduced spelling- or "
            f"formatting-dependent verdicts ([CHKARCH-CONFORMANCE-MUTATION]).",
            file=sys.stderr,
        )
        return 1
    print(f"ratchet OK: {rate:.1%} >= floor {floor:.1%}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
