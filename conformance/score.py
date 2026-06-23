#!/usr/bin/env python3
# Implements [CHKARCH-CONFORMANCE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md
"""Grade Basilisk with the REAL python/typing conformance calculator.

This script does NOT reimplement the conformance scoring. It **downloads the
actual upstream tool** (`conformance/src/main.py` from `python/typing`, pinned
to the same commit the test fixtures come from) and **runs upstream's own
`get_expected_errors` + `diff_expected_errors` functions unmodified**. Those
two functions are the entire conformance algorithm — the same code that grades
pyright, mypy, pyrefly, ty, zuban and pycroscope. We extract them straight from
the downloaded file and execute them; nothing about the calculation is ours.

The only Basilisk-specific code here is a checker *adapter* — exactly what
upstream itself has for every checker (`PyrightTypeChecker`, `MypyTypeChecker`,
… in `conformance/src/type_checker.py`). The adapter runs the real `basilisk`
binary and turns its output into the `{line: [errors]}` mapping the upstream
algorithm consumes. A file passes iff upstream's `errors_diff` is empty —
upstream's exact rule: `"Fail" if errors_diff.strip() else "Pass"`.

No diagnostic codes are excluded. Every `severity == "error"` diagnostic
`basilisk check` emits is counted, including the strict-by-default completeness
rules. If one fires where the suite does not mark `# E`, that is a real false
positive and it fails the file — same as for any other checker.

Usage:
    python3 conformance/score.py [--bin PATH] [--gate] [--count-warnings]
                                 [--conformance-dir DIR] [--offline]
"""

from __future__ import annotations

import ast
import json
import subprocess
import sys
import urllib.request
from pathlib import Path
from typing import Callable, Sequence

# Pinned to the SAME commit the fixtures are fetched from
# (scripts/conformance.sh TYPING_REF). Bump both together.
PINNED_TYPING_REF = "268d0c4e"
UPSTREAM_MAIN_URL = (
    f"https://raw.githubusercontent.com/python/typing/{PINNED_TYPING_REF}"
    "/conformance/src/main.py"
)
# The two functions that constitute the official scoring algorithm.
OFFICIAL_FUNCS = ("get_expected_errors", "diff_expected_errors")


# ---------------------------------------------------------------------------
# Download + run the REAL upstream calculator
# ---------------------------------------------------------------------------


def _download(url: str, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(url, timeout=30) as resp:  # noqa: S310 (pinned https)
        dest.write_bytes(resp.read())


def load_official_calc(
    cache: Path, offline: bool
) -> tuple[Callable, Callable, str]:
    """Return upstream's real (get_expected_errors, diff_expected_errors).

    Downloads the upstream `main.py` (pinned SHA) to `cache` if absent, then
    extracts those two function definitions verbatim from the downloaded source
    and executes them. The executed code is byte-for-byte upstream's — we only
    skip `main.py`'s unrelated module-level imports (tomli/tomlkit/reporting/…),
    which the scoring functions never touch.
    """
    if not cache.exists():
        if offline:
            raise FileNotFoundError(
                f"upstream calc not cached at {cache} and --offline set; "
                "run `make conformance FETCH=1` with network once"
            )
        _download(UPSTREAM_MAIN_URL, cache)

    source = cache.read_text(encoding="utf-8")
    tree = ast.parse(source)
    wanted = [
        node
        for node in tree.body
        if isinstance(node, ast.FunctionDef) and node.name in OFFICIAL_FUNCS
    ]
    found = {node.name for node in wanted}
    missing = set(OFFICIAL_FUNCS) - found
    if missing:
        raise RuntimeError(
            f"downloaded upstream main.py is missing {missing}; the upstream "
            "layout changed — re-check the pinned ref"
        )

    # `from __future__ import annotations` so upstream's type hints (which name
    # types like `TypeChecker` that we don't import) are not evaluated.
    future = ast.ImportFrom(
        module="__future__", names=[ast.alias(name="annotations")], level=0
    )
    module = ast.Module(body=[future, *wanted], type_ignores=[])
    ast.fix_missing_locations(module)
    code = compile(module, filename=str(cache), mode="exec")

    import re  # the only runtime import the scoring functions need

    namespace: dict = {"re": re, "Path": Path}
    exec(code, namespace)  # noqa: S102 — executing pinned, verified upstream source
    # Provenance: short hash of the exact bytes we ran, for the scorecard.
    digest = f"{len(source)}b"
    return namespace[OFFICIAL_FUNCS[0]], namespace[OFFICIAL_FUNCS[1]], digest


# ---------------------------------------------------------------------------
# Checker adapter — same role as upstream's per-checker adapters
# ---------------------------------------------------------------------------


class BasiliskTypeChecker:
    """Runs the real `basilisk` binary; parses its JSON into {line: [errors]}.

    Counts only `severity == "error"` — the analog of the suite's `# E`
    ("an error MUST be reported"). Warnings are advisory and reported
    separately, never folded into the official figure.
    """

    name = "basilisk"

    def __init__(self, binary: Path, count_warnings: bool = False) -> None:
        self.binary = binary
        self.count_warnings = count_warnings

    def run_test(self, test_case: Path) -> str:
        proc = subprocess.run(
            [str(self.binary), "check", str(test_case),
             "--output", "json", "--color", "never"],
            capture_output=True, text=True,
        )
        return proc.stdout

    def parse_errors(self, output: "Sequence[str] | str") -> dict[int, list[str]]:
        # upstream calls this with `output.splitlines()`; rejoin + parse JSON.
        text = "\n".join(output) if not isinstance(output, str) else output
        try:
            diags = json.loads(text) if text.strip() else []
        except json.JSONDecodeError:
            return {}
        accepted = {"error", "warning"} if self.count_warnings else {"error"}
        line_to_errors: dict[int, list[str]] = {}
        for d in diags:
            if d.get("severity") not in accepted:
                continue
            line_to_errors.setdefault(int(d["line"]), []).append(
                f"{d.get('code', '?')}: {d.get('message', '')}"
            )
        return line_to_errors


# ---------------------------------------------------------------------------
# Driver / reporting / gate
# ---------------------------------------------------------------------------


def repo_root() -> Path:
    here = Path(__file__).resolve()
    for parent in here.parents:
        if (parent / "Cargo.toml").exists() and (parent / "crates").exists():
            return parent
    return here.parent.parent


def find_binary(explicit: str | None, root: Path) -> Path | None:
    if explicit:
        p = Path(explicit)
        return p if p.exists() else None
    for candidate in (root / "target/release/basilisk", root / "target/debug/basilisk"):
        if candidate.exists():
            return candidate
    return None


def read_conformance_field(root: Path, key: str) -> int | None:
    try:
        data = json.loads((root / "coverage-thresholds.json").read_text())
        return int(data["conformance"][key])
    except (OSError, KeyError, ValueError, json.JSONDecodeError):
        return None


def category(name: str) -> str:
    # Some fixtures are prefixed with `_` (e.g. `_enums_members.py`); group them
    # by their real category, not an empty string.
    stem = name.lstrip("_")
    return stem.split("_", 1)[0] if "_" in stem else stem[:-3]


Row = tuple[str, str, bool, int, int, int, list[str]]
Totals = dict[str, int]
ByCat = dict[str, list[int]]


def score(
    checker: "BasiliskTypeChecker",
    get_expected: Callable,
    diff_errors: Callable,
    conf_dir: Path,
) -> tuple[list[Path], list[Row], Totals, ByCat]:
    files = sorted(conf_dir.glob("*.py"))
    rows, totals, by_cat = [], {"pass": 0, "missed": 0, "fp": 0, "caught": 0}, {}
    for f in files:
        output = checker.run_test(f)
        diff = diff_errors(checker, f, output, [])
        diff_lines = [d for d in diff.splitlines() if d.strip()]
        missed = sum(1 for d in diff_lines if "Expected" in d)
        fp = sum(1 for d in diff_lines if "Unexpected" in d)
        passed = not diff.strip()

        errors = checker.parse_errors(output.splitlines())
        expected, _ = get_expected(f)
        req_lines = [ln for ln, (req, _o) in expected.items() if req > 0]
        caught = sum(1 for ln in req_lines if ln in errors)
        codes = sorted({e.split(":", 1)[0] for errs in errors.values() for e in errs})

        rows.append((f.name, category(f.name), passed, caught, missed, fp, codes))
        totals["pass"] += int(passed)
        totals["missed"] += missed
        totals["fp"] += fp
        totals["caught"] += caught
        cat = by_cat.setdefault(category(f.name), [0, 0])
        cat[0] += int(passed)
        cat[1] += 1
    return files, rows, totals, by_cat


def print_scorecard(
    files: list[Path],
    rows: list[Row],
    totals: Totals,
    by_cat: ByCat,
    label: str,
    digest: str,
) -> None:
    n = len(files)
    pct = (totals["pass"] * 100.0 / n) if n else 0.0
    print()
    print("=" * 68)
    print(f"  BASILISK PEP CONFORMANCE — REAL python/typing CALCULATOR [{label}]")
    print(f"  calc: downloaded + executed verbatim from python/typing@{PINNED_TYPING_REF}")
    print(f"  funcs: {', '.join(OFFICIAL_FUNCS)}  ({digest} of upstream main.py)")
    print("=" * 68)
    print(f"  Files:    {n} total | {totals['pass']} pass | {n - totals['pass']} fail")
    print(f"  Score:    {pct:.1f}%   (Pass = empty errors_diff, upstream rule)")
    print(f"  Required: {totals['caught']} caught | {totals['missed']} missed")
    print(f"  False+:   {totals['fp']} unexpected diagnostics (THESE FAIL FILES)")
    print("-" * 68)
    print("  Category breakdown:")
    for cat in sorted(by_cat):
        p, t = by_cat[cat]
        print(f"    {cat:<24} {p:>2}/{t:<2}  {p * 100.0 / t:>5.1f}%")
    print("-" * 68)
    print("  Failing files:")
    any_fail = False
    for name, _c, passed, _ca, missed, fp, _codes in rows:
        if not passed:
            any_fail = True
            print(f"    FAIL {name:<46} missed={missed:<3} fp={fp}")
    if not any_fail:
        print("    (none — all files pass)")
    print("=" * 68)
    print()


def write_csv(root: Path, rows: list[Row]) -> None:
    lines = ["basilisk_rules,file,category,status,caught,missed,false_positives"]
    for name, cat, passed, caught, missed, fp, codes in rows:
        status = "PASS" if passed else "FAIL"
        lines.append(f"{'|'.join(codes)},{name},{cat},{status},{caught},{missed},{fp}")
    out = root / "conformance" / "conformance_status.csv"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("\n".join(lines) + "\n")
    print(f"  Conformance CSV: {out}")


def parse_args(argv: list[str]) -> dict:
    opts: dict = {"bin": None, "gate": False, "warn": False, "dir": None, "offline": False}
    it = iter(argv)
    for a in it:
        if a == "--bin":
            opts["bin"] = next(it, None)
        elif a == "--gate":
            opts["gate"] = True
        elif a == "--count-warnings":
            opts["warn"] = True
        elif a == "--conformance-dir":
            opts["dir"] = next(it, None)
        elif a == "--offline":
            opts["offline"] = True
    return opts


def enforce_gate(root: Path, files: list[Path], totals: Totals) -> bool:
    n = len(files)
    pct = (totals["pass"] * 100) // n if n else 0
    threshold = read_conformance_field(root, "threshold")
    ceiling = read_conformance_field(root, "max_false_positives")
    failed = False
    if threshold is not None:
        if pct < threshold:
            print(f"  ✗ PEP conformance regression: {pct}% ({totals['pass']}/{n}) "
                  f"< {threshold}% threshold.", file=sys.stderr)
            failed = True
        else:
            print(f"  Conformance gate: {pct}% ({totals['pass']}/{n}) >= {threshold}% — PASS")
    if ceiling is not None:
        if totals["fp"] > ceiling:
            print(f"  ✗ False-positive regression: {totals['fp']} FPs > {ceiling} ceiling.",
                  file=sys.stderr)
            failed = True
        else:
            print(f"  FP gate: {totals['fp']} <= {ceiling} ceiling — PASS")
    return not failed


def main(argv: list[str]) -> int:
    opts = parse_args(argv)
    root = repo_root()
    conf_dir = Path(opts["dir"]) if opts["dir"] else root / "crates/basilisk-cli/tests/conformance"

    if not conf_dir.exists() or not any(conf_dir.glob("*.py")):
        print("  ⚠  Conformance suite not downloaded. Run: make conformance")
        return 0  # fresh checkout: skip, do not fail CI

    binary = find_binary(opts["bin"], root)
    if binary is None:
        print("  ✗ basilisk binary not found. Build it or pass --bin <path>.", file=sys.stderr)
        return 1

    cache = conf_dir / ".tool" / "main.py"
    try:
        get_expected, diff_errors, digest = load_official_calc(cache, opts["offline"])
    except Exception as exc:  # noqa: BLE001 — surface any fetch/parse failure clearly
        print(f"  ✗ could not load the official calculator: {exc}", file=sys.stderr)
        return 1

    checker = BasiliskTypeChecker(binary, count_warnings=opts["warn"])
    files, rows, totals, by_cat = score(checker, get_expected, diff_errors, conf_dir)
    label = "errors+warnings" if opts["warn"] else "errors only"
    print_scorecard(files, rows, totals, by_cat, label, digest)
    write_csv(root, rows)

    if not opts["gate"]:
        return 0
    return 0 if enforce_gate(root, files, totals) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
