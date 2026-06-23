#!/usr/bin/env python3
# Implements [CHKARCH-CONFORMANCE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md
"""Grade Basilisk with the REAL python/typing conformance calculator.

This script does NOT reimplement the conformance scoring. It **imports the
committed upstream tool** — `conformance/upstream_main.py`, a byte-identical,
sha256-verified copy of `conformance/src/main.py` from `python/typing` pinned to
the same commit the test fixtures come from — and **calls upstream's own
`get_expected_errors` + `diff_expected_errors` functions unmodified**. Those two
functions are the entire conformance algorithm: the same code that grades
pyright, mypy, pyrefly, ty, zuban and pycroscope. Nothing about the calculation
is ours, and nothing is downloaded at score time.

The only Basilisk-specific code here is a checker *adapter* — exactly what
upstream itself has for every checker (`PyrightTypeChecker`, `MypyTypeChecker`,
… in `conformance/src/type_checker.py`). The adapter runs the real `basilisk`
binary and turns its output into the `{line: [errors]}` mapping the upstream
algorithm consumes. A file passes iff upstream's `errors_diff` is empty —
upstream's exact rule: `"Fail" if errors_diff.strip() else "Pass"`.

No diagnostic codes are excluded. By default this counts EVERY diagnostic
`basilisk check` emits — both errors AND warnings — which is the strictest
grading and matches how the reference checker pyright is graded upstream
(`if kind not in ("error", "warning")`). Pass `--errors-only` for the looser
errors-only view. Either way, any diagnostic on a line the suite does not mark
`# E` is a real false positive and fails the file — same as for any checker.

This one file is the whole Basilisk side of conformance: it runs the binary,
scores with the official functions, writes `conformance/conformance_status.csv`,
and enforces the ratchet gate (`--gate`). There is no Rust test and no shell
script. Everything it needs is committed to the repo — nothing is a moving
target, nothing is fetched at score time:
  • the official calculator → `conformance/upstream_main.py` (sha256-pinned)
  • the `# E`-annotated test fixtures → `conformance/tests/*.py`

Both are refreshed ONLY as deliberate maintenance, then committed:
    python3 conformance/score.py --refresh-upstream   # re-pin the calculator
    python3 conformance/score.py --fetch              # re-download the fixtures

Usage:
    python3 conformance/score.py [--bin PATH] [--gate] [--errors-only]
                                 [--conformance-dir DIR] [--fetch | --fetch-only]
                                 [--refresh-upstream]
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import subprocess
import sys
import types
from pathlib import Path
from typing import Callable, Sequence

# The single home for the pinned upstream commit. The fixtures (FIXTURES_API)
# and the vendored calculator (UPSTREAM_MAIN) both track it. To bump: edit this,
# run `--refresh-upstream` (re-pins upstream_main.py + its sha256), then `--fetch`.
PINNED_TYPING_REF = "268d0c4e"
UPSTREAM_MAIN_URL = (
    f"https://raw.githubusercontent.com/python/typing/{PINNED_TYPING_REF}"
    "/conformance/src/main.py"
)
# The committed, byte-identical copy of upstream's calculator, and its sha256.
UPSTREAM_MAIN = Path(__file__).resolve().parent / "upstream_main.py"
UPSTREAM_MAIN_SHA256 = "b4e3bd089c73856f9920ef494350d622c2914fac238c9193ec0bb3f93f0fc6a2"
# The two functions that constitute the official scoring algorithm.
OFFICIAL_FUNCS = ("get_expected_errors", "diff_expected_errors")
# The `# E`-annotated test fixtures are committed under conformance/tests. This
# API lists them at the pinned ref for the maintenance-only `--fetch` refresh.
FIXTURES_API = (
    "https://api.github.com/repos/python/typing/contents/conformance/tests"
    f"?ref={PINNED_TYPING_REF}"
)


# ---------------------------------------------------------------------------
# Import the REAL upstream calculator (committed, sha256-verified — no download)
# ---------------------------------------------------------------------------


def _stub_module(name: str, **attrs: object) -> None:
    """Register an empty stand-in module so upstream's unrelated top-level
    imports resolve. The two scoring functions touch none of these."""
    module = types.ModuleType(name)
    for attr, value in attrs.items():
        setattr(module, attr, value)
    sys.modules[name] = module


def load_official_calc() -> tuple[Callable, Callable, str]:
    """Return upstream's real (get_expected_errors, diff_expected_errors).

    Reads the committed `conformance/upstream_main.py`, verifies it is byte-for-
    byte the pinned upstream `conformance/src/main.py` (sha256), imports it, and
    hands back its two functions unmodified. No network access; no code of ours
    in the calculation. `main.py` also imports tomli/tomlkit/options/reporting/
    test_groups/type_checker at module scope — the scoring functions use none of
    them, so empty stubs let the import succeed.
    """
    raw = UPSTREAM_MAIN.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != UPSTREAM_MAIN_SHA256:
        raise RuntimeError(
            f"{UPSTREAM_MAIN.name} sha256 {digest[:12]}… != pinned "
            f"{UPSTREAM_MAIN_SHA256[:12]}… — the vendored upstream calculator was "
            "modified. Restore it from git, or run --refresh-upstream to re-pin."
        )

    _stub_module("tomli")
    _stub_module("tomlkit")
    _stub_module("options", parse_options=None)
    _stub_module("reporting", generate_summary=None)
    _stub_module("test_groups", get_test_cases=None, get_test_groups=None)
    _stub_module("type_checker", TYPE_CHECKERS=(), TypeChecker=object)

    spec = importlib.util.spec_from_file_location("typing_conformance_main", UPSTREAM_MAIN)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot build an import spec for {UPSTREAM_MAIN}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)

    missing = [name for name in OFFICIAL_FUNCS if not hasattr(module, name)]
    if missing:
        raise RuntimeError(
            f"committed upstream main.py is missing {missing}; the upstream "
            "layout changed — re-check the pinned ref"
        )
    get_expected = getattr(module, OFFICIAL_FUNCS[0])
    diff_errors = getattr(module, OFFICIAL_FUNCS[1])
    return get_expected, diff_errors, f"sha256:{digest[:12]}"


def refresh_upstream() -> int:
    """Re-download upstream main.py to the committed path and print its sha256.

    Maintenance only — run when bumping PINNED_TYPING_REF. This is the ONLY code
    path that touches the network; the normal score path never does.
    """
    import urllib.request  # local import: never loaded on the score path

    with urllib.request.urlopen(UPSTREAM_MAIN_URL, timeout=30) as resp:  # noqa: S310 (pinned https)
        raw = resp.read()
    UPSTREAM_MAIN.write_bytes(raw)
    digest = hashlib.sha256(raw).hexdigest()
    print(f"  fetched {UPSTREAM_MAIN_URL}")
    print(f"  wrote   {UPSTREAM_MAIN} ({len(raw)} bytes)")
    print(f"  sha256  {digest}")
    if digest != UPSTREAM_MAIN_SHA256:
        print(f'  -> update UPSTREAM_MAIN_SHA256 = "{digest}" (ref changed)')
    return 0


# ---------------------------------------------------------------------------
# MAINTENANCE: re-download the committed test fixtures (run periodically, commit)
# ---------------------------------------------------------------------------


def ensure_fixtures(conf_dir: Path, force: bool) -> None:
    """Download python/typing's conformance `.py` fixtures into `conf_dir`.

    The fixtures are COMMITTED to the repo — this is a maintenance helper invoked
    only by `--fetch` / `--fetch-only`, after which the result is committed. The
    normal score path never calls it. No-op when already present at the pinned ref
    (a `.ref-sha` stamp records it) unless `force`. Honors `GITHUB_TOKEN` to raise
    the API rate limit.
    """
    import os
    import urllib.request  # local: network only happens here and in refresh

    stamp = conf_dir / ".ref-sha"
    cached_ref = stamp.read_text(encoding="utf-8").strip() if stamp.exists() else ""
    present = conf_dir.exists() and any(conf_dir.glob("*.py"))
    if present and cached_ref == PINNED_TYPING_REF and not force:
        return

    headers = {"Accept": "application/vnd.github+json"}
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        headers["Authorization"] = f"token {token}"

    listing_req = urllib.request.Request(FIXTURES_API, headers=headers)
    with urllib.request.urlopen(listing_req, timeout=60) as resp:  # noqa: S310 (pinned https)
        entries = json.loads(resp.read())
    fixtures = [e for e in entries if e.get("type") == "file" and e["name"].endswith(".py")]
    if not fixtures:
        raise RuntimeError(f"no .py fixtures found at {FIXTURES_API}")

    conf_dir.mkdir(parents=True, exist_ok=True)
    for stale in conf_dir.glob("*.py"):
        stale.unlink()
    for entry in fixtures:
        with urllib.request.urlopen(entry["download_url"], timeout=60) as resp:  # noqa: S310
            (conf_dir / entry["name"]).write_bytes(resp.read())
    stamp.write_text(PINNED_TYPING_REF + "\n", encoding="utf-8")
    print(f"  fetched {len(fixtures)} conformance fixtures "
          f"(python/typing@{PINNED_TYPING_REF}) -> {conf_dir}")


# ---------------------------------------------------------------------------
# Checker adapter — same role as upstream's per-checker adapters
# ---------------------------------------------------------------------------


class BasiliskTypeChecker:
    """Runs the real `basilisk` binary; parses its JSON into {line: [errors]}.

    Each diagnostic is the analog of the suite's `# E` ("an error MUST be
    reported on this line"). When `count_warnings` is set (the default for the
    strictest grading, matching how pyright is graded upstream) both `error` and
    `warning` severities count; otherwise only `error` does.
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
    print("  calc: imported verbatim from committed conformance/upstream_main.py")
    print(f"  ref:  python/typing@{PINNED_TYPING_REF}  ({digest})  funcs: {', '.join(OFFICIAL_FUNCS)}")
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
    # Default is the STRICTEST grading: every diagnostic basilisk emits (errors
    # AND warnings) is counted as "an error was reported", which is also how the
    # reference checker pyright is graded upstream. `--errors-only` reports the
    # looser errors-only view. `--count-warnings` is accepted for back-compat.
    opts: dict = {
        "bin": None, "gate": False, "warn": True, "dir": None,
        "refresh": False, "fetch": False, "fetch_only": False,
    }
    it = iter(argv)
    for a in it:
        if a == "--bin":
            opts["bin"] = next(it, None)
        elif a == "--gate":
            opts["gate"] = True
        elif a == "--count-warnings":
            opts["warn"] = True
        elif a == "--errors-only":
            opts["warn"] = False
        elif a == "--conformance-dir":
            opts["dir"] = next(it, None)
        elif a == "--refresh-upstream":
            opts["refresh"] = True
        elif a == "--fetch":
            opts["fetch"] = True
        elif a == "--fetch-only":
            opts["fetch_only"] = True
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
    if opts["refresh"]:
        return refresh_upstream()

    root = repo_root()
    conf_dir = Path(opts["dir"]) if opts["dir"] else root / "conformance/tests"

    # MAINTENANCE ONLY: --fetch / --fetch-only re-download the fixtures so they can
    # be committed. The fixtures are committed to the repo (not a moving target);
    # the normal score path NEVER touches the network.
    if opts["fetch"] or opts["fetch_only"]:
        try:
            ensure_fixtures(conf_dir, force=True)
        except Exception as exc:  # noqa: BLE001 — surface fetch failure clearly
            print(f"  ✗ could not fetch conformance fixtures: {exc}", file=sys.stderr)
            return 1
        if opts["fetch_only"]:
            return 0

    if not conf_dir.exists() or not any(conf_dir.glob("*.py")):
        print("  ✗ conformance fixtures missing at conformance/tests/. They are "
              "committed to the repo; restore them from git, or run "
              "`python3 conformance/score.py --fetch` to re-download then commit.",
              file=sys.stderr)
        return 1

    binary = find_binary(opts["bin"], root)
    if binary is None:
        print("  ✗ basilisk binary not found. Build it or pass --bin <path>.", file=sys.stderr)
        return 1

    try:
        get_expected, diff_errors, digest = load_official_calc()
    except Exception as exc:  # noqa: BLE001 — surface any load/verify failure clearly
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
