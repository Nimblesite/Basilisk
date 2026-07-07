#!/usr/bin/env python3
# Implements [CHKARCH-CONFORMANCE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md
"""Grade Basilisk with the REAL python/typing conformance calculator.

This script does NOT reimplement the conformance scoring. It tracks the LATEST
`python/typing@main`: every run resolves the current `main` tip, refreshes the
vendored calculator (`conformance/upstream_main.py`, a byte-identical copy of
`conformance/src/main.py`) and the `# E`-annotated fixtures when `main` has
moved, records the resolved commit + the calculator's sha256 in the website
report, then **calls upstream's own `get_expected_errors` +
`diff_expected_errors` functions unmodified**. Those two functions are the entire
conformance algorithm: the same code that grades pyright, mypy, pyrefly, ty,
zuban and pycroscope. Nothing about the calculation is ours. When `main` cannot
be reached the cached fixtures are scored instead and the result is flagged
`stale`.

The only Basilisk-specific code here is a checker *adapter* — exactly what
upstream itself has for every checker (`PyrightTypeChecker`, `MypyTypeChecker`,
… in `conformance/src/type_checker.py`). The adapter runs the real `basilisk`
binary and turns its output into the `{line: [errors]}` mapping the upstream
algorithm consumes. A file passes iff upstream's `errors_diff` is empty —
upstream's exact rule: `"Fail" if errors_diff.strip() else "Pass"`.

Nothing is excluded from *scoring*: the calculator counts EVERY diagnostic
`basilisk check` emits — both errors AND warnings — which is the strictest
grading and matches how the reference checker pyright is graded upstream
(`if kind not in ("error", "warning")`). There is exactly ONE grading, applied
on every run — no looser mode and no opt-out. Any diagnostic on a line the suite
does not mark `# E` is a real false positive and fails the file — same as for
any checker.

⚠️ DISABLING ANY RULE FOR CONFORMANCE SCORING IS FORBIDDEN. ⚠️
The binary is run in its FULL, DEFAULT, strict-by-default configuration with
EVERY rule enabled — no `basilisk.json`, no `--disable`, no per-rule override, no
"spec-conformance mode", no exceptions, no matter what. Before scoring, this
script DELETES any `basilisk.json` from the fixtures directory so a stale config
can never silence a rule. The conformance number must reflect exactly what a real
user gets out of the box. If basilisk's strict defaults flag valid type-system
code, that is a REAL conformance gap to FIX in the Rust checker — never to hide by
turning a rule off. The honest number is the only number. See [CHKARCH-CONFORMANCE].

This one file is the whole Basilisk side of conformance: it tracks `main`, runs
the binary, scores with the official functions, writes
`conformance/conformance_status.csv` and the website report
(`website/src/_data/conformance_report.json`, which records the resolved commit
and calculator hash), and enforces the ratchet gate (`--gate`). The fixtures and
the vendored calculator both track `python/typing@main` and are git-ignored /
refreshed on demand; the report and CSV are committed so the website builds
without running the scorer.

Usage:
    python3 conformance/score.py [--bin PATH] [--gate]
                                 [--conformance-dir DIR] [--fetch-only]
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

# We track the LATEST `python/typing@main` rather than freezing a commit: every
# run resolves the current `main` tip, re-fetches the fixtures + calculator when
# it has moved, and records the resolved commit in the website report below.
UPSTREAM_REPO = "python/typing"
UPSTREAM_REF = "main"
# Resolve `main` -> commit metadata; list the fixtures dir; fetch raw files.
COMMITS_API = f"https://api.github.com/repos/{UPSTREAM_REPO}/commits/{UPSTREAM_REF}"
CONTENTS_API = (
    f"https://api.github.com/repos/{UPSTREAM_REPO}/contents/conformance/tests"
)
RAW_MAIN_AT = (
    f"https://raw.githubusercontent.com/{UPSTREAM_REPO}/{{sha}}/conformance/src/main.py"
)
# The vendored copy of upstream's calculator (refreshed from the resolved commit).
UPSTREAM_MAIN = Path(__file__).resolve().parent / "upstream_main.py"
# Records which commit the fixtures/calculator were last fetched at.
REF_STAMP = Path(__file__).resolve().parent / "tests" / ".ref-sha"
# The two functions that constitute the official scoring algorithm.
OFFICIAL_FUNCS = ("get_expected_errors", "diff_expected_errors")
# Machine-readable report consumed by the website build (Eleventy global
# `conformanceReport`): the resolved upstream commit, the calculator hash, and
# the full score. Written on every run so the site never re-derives or hard-codes
# the number — see website/src/_data/conformance.js.
WEBSITE_REPORT = (
    Path(__file__).resolve().parent.parent
    / "website"
    / "src"
    / "_data"
    / "conformance_report.json"
)


# ---------------------------------------------------------------------------
# Import the REAL upstream calculator (committed, sha256-verified — no download)
# ---------------------------------------------------------------------------


class _StubModule(types.ModuleType):
    """Stand-in that resolves ANY attribute to a dummy, so upstream main.py's
    unrelated top-level imports (tomli/tomlkit/options/reporting/test_groups/
    type_checker) succeed. The two scoring functions reference none of them."""

    def __getattr__(self, _name: str) -> object:
        return object


def load_official_calc() -> tuple[Callable, Callable, str]:
    """Return upstream's real (get_expected_errors, diff_expected_errors).

    Reads the vendored `conformance/upstream_main.py` (refreshed from the
    resolved `main` commit), imports it behind module stubs (above), and hands
    back its two functions unmodified. The file's sha256 is recorded in the
    website report so the build can re-verify it; nothing of ours touches the
    calculation.
    """
    raw = UPSTREAM_MAIN.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()

    for dep in (
        "tomli",
        "tomlkit",
        "options",
        "reporting",
        "test_groups",
        "type_checker",
    ):
        sys.modules.setdefault(dep, _StubModule(dep))

    spec = importlib.util.spec_from_file_location(
        "typing_conformance_main", UPSTREAM_MAIN
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot build an import spec for {UPSTREAM_MAIN}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)

    funcs = tuple(getattr(module, name, None) for name in OFFICIAL_FUNCS)
    if not all(funcs):
        raise RuntimeError(
            f"vendored upstream main.py is missing {OFFICIAL_FUNCS}; the upstream "
            "layout changed at python/typing@main"
        )
    return funcs[0], funcs[1], f"sha256:{digest[:12]}"


# ---------------------------------------------------------------------------
# Track python/typing@main: resolve the tip and (re)download fixtures+calculator
# ---------------------------------------------------------------------------


def _gh_headers() -> dict:
    """GitHub API headers, honouring `GITHUB_TOKEN` to raise the rate limit."""
    import os

    headers = {"Accept": "application/vnd.github+json"}
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        headers["Authorization"] = f"token {token}"
    return headers


def _urlopen_retry(target: object, *, timeout: int, retries: int = 8) -> bytes:
    """`urlopen(target).read()` with exponential backoff on HTTP 429 / 5xx.

    Refreshing the suite fires ~150 requests at raw.githubusercontent.com in a
    burst, and GitHub's shared-runner-IP rate limit answers 429 partway through.
    Retrying each request with backoff absorbs a transient throttle so the CI
    score is reliable. This hardens ONLY how bytes are fetched — it changes
    nothing about how files are scored (the official calculator is untouched).
    """
    import time
    import urllib.error
    import urllib.request

    delay = 1.0
    last: Exception = RuntimeError("no request attempted")
    for _ in range(retries):
        try:
            with urllib.request.urlopen(target, timeout=timeout) as resp:  # noqa: S310
                return resp.read()
        except urllib.error.HTTPError as exc:  # noqa: PERF203 — retry transient codes only
            last = exc
            if exc.code not in (429, 500, 502, 503, 504):
                raise
        except urllib.error.URLError as exc:
            last = exc
        time.sleep(delay)
        delay = min(delay * 2, 20.0)
    raise last


def resolve_upstream_commit() -> dict:
    """Resolve the current `python/typing@main` tip to its commit metadata."""
    import urllib.request

    req = urllib.request.Request(COMMITS_API, headers=_gh_headers())
    data = json.loads(_urlopen_retry(req, timeout=30))
    sha = data["sha"]
    date = data.get("commit", {}).get("committer", {}).get("date", "")[:10]
    return {"sha": sha, "short": sha[:7], "date": date}


def download_fixtures(conf_dir: Path, sha: str) -> int:
    """Download every `.py`/`.pyi` fixture from `conformance/tests` at `sha`.

    Fetches BOTH the `.py` tests AND the `.pyi` support stubs they import (e.g.
    `qualifiers_final_decorator.py` imports `_qualifiers_final_decorator`). Only
    non-`_` `.py`/`.pyi` files are SCORED (see `score()`); the rest are inputs.

    The download is ATOMIC: fixtures land in a sibling staging dir and are
    swapped into `conf_dir` only once EVERY file has arrived. A 429 (or any
    error) partway through therefore leaves the existing cached fixtures intact
    instead of purging them into a partial set that would later score as a false
    regression.
    """
    import shutil
    import tempfile
    import urllib.request

    listing = f"{CONTENTS_API}?ref={sha}"
    req = urllib.request.Request(listing, headers=_gh_headers())
    entries = json.loads(_urlopen_retry(req, timeout=60))
    fixtures = [
        e
        for e in entries
        if e.get("type") == "file" and e["name"].endswith((".py", ".pyi"))
    ]
    if not fixtures:
        raise RuntimeError(f"no .py/.pyi fixtures at {listing}")

    conf_dir.mkdir(parents=True, exist_ok=True)
    # Stage inside `conf_dir` (a dotfile subdir): same filesystem, so the per-file
    # swap below is an atomic rename, and it stays within the git-ignored fixtures
    # tree — `score()` globs `*.py`/`*.pyi` files, never this subdirectory.
    staging = Path(tempfile.mkdtemp(prefix=".fixtures-", dir=conf_dir))
    try:
        for entry in fixtures:
            (staging / entry["name"]).write_bytes(
                _urlopen_retry(entry["download_url"], timeout=60)
            )
        # Every fixture landed — swap atomically: drop the old set, move new in.
        for stale in (*conf_dir.glob("*.py"), *conf_dir.glob("*.pyi")):
            stale.unlink()
        for staged in staging.iterdir():
            staged.replace(conf_dir / staged.name)
    finally:
        shutil.rmtree(staging, ignore_errors=True)
    return len(fixtures)


def download_calculator(sha: str) -> None:
    """Refresh the vendored calculator from `conformance/src/main.py` at `sha`."""
    UPSTREAM_MAIN.write_bytes(_urlopen_retry(RAW_MAIN_AT.format(sha=sha), timeout=30))


def fetch_upstream(conf_dir: Path) -> dict:
    """Track `python/typing@main`: resolve its tip, refresh fixtures + calculator.

    Every run pulls the latest `main` — CI and release included — so the score is
    always against the current upstream suite: it resolves the tip and
    re-downloads the ~150 fixtures + the calculator unconditionally. On any
    network failure the cached fixtures are scored instead, flagged `stale`.
    Returns the resolved commit plus the calculator hash/size for the report.
    Honors `GITHUB_TOKEN`.
    """
    try:
        commit = resolve_upstream_commit()
    except Exception as exc:  # noqa: BLE001 — degrade to cache, never crash the score
        return _cached_commit(conf_dir, exc, None)

    try:
        count = download_fixtures(conf_dir, commit["sha"])
        download_calculator(commit["sha"])
        REF_STAMP.parent.mkdir(parents=True, exist_ok=True)
        REF_STAMP.write_text(commit["sha"] + "\n", encoding="utf-8")
        print(
            f"  fetched {count} fixtures + calculator (python/typing@{commit['short']})"
        )
    except Exception as exc:  # noqa: BLE001
        return _cached_commit(conf_dir, exc, commit)

    return _with_calculator_hash(commit, stale=False)


def _with_calculator_hash(commit: dict, *, stale: bool) -> dict:
    raw = UPSTREAM_MAIN.read_bytes() if UPSTREAM_MAIN.exists() else b""
    return {
        **commit,
        "calculator_sha256": hashlib.sha256(raw).hexdigest() if raw else "",
        "calculator_bytes": len(raw),
        "stale": stale,
    }


def _cached_commit(conf_dir: Path, exc: Exception, commit: dict | None) -> dict:
    """Fall back to cached fixtures when `main` cannot be reached."""
    present = conf_dir.exists() and any(conf_dir.glob("*.py"))
    if not present:
        raise RuntimeError(
            f"cannot reach python/typing@{UPSTREAM_REF} and no cached fixtures: {exc}"
        )
    cached_sha = (
        REF_STAMP.read_text(encoding="utf-8").strip() if REF_STAMP.exists() else ""
    )
    sha = (commit or {}).get("sha") or cached_sha
    print(
        f"  ⚠  could not reach python/typing@{UPSTREAM_REF} ({exc}); "
        "scoring cached fixtures"
    )
    base = {
        "sha": sha,
        "short": (commit or {}).get("short") or (sha[:7] if sha else "unknown"),
        "date": (commit or {}).get("date", ""),
    }
    return _with_calculator_hash(base, stale=True)


def write_website_report(commit: dict, rows: list[Row], totals: Totals, n: int) -> None:
    """Write the machine-readable report the website build consumes."""
    pct = round(totals["pass"] * 100.0 / n, 1) if n else 0.0
    report = {
        "_doc": (
            "Generated by conformance/score.py on every run. The website build "
            "(website/src/_data/conformance.js) reads this for the upstream commit "
            "and calculator hash. Do not hand-edit."
        ),
        "upstream": {
            "repo": UPSTREAM_REPO,
            "ref": UPSTREAM_REF,
            "sha": commit.get("sha", ""),
            "shortSha": commit.get("short", ""),
            "commitDate": commit.get("date", ""),
            "stale": commit.get("stale", False),
        },
        "calculator": {
            "file": "conformance/upstream_main.py",
            "sha256": commit.get("calculator_sha256", ""),
            "bytes": commit.get("calculator_bytes", 0),
            "funcs": list(OFFICIAL_FUNCS),
        },
        "grading": "errors+warnings, every rule enabled (strict default config)",
        "score": {
            "pass": totals["pass"],
            "total": n,
            "fail": n - totals["pass"],
            "scorePct": pct,
            "caught": totals["caught"],
            "missed": totals["missed"],
            "falsePositives": totals["fp"],
        },
        "files": [
            {
                "file": name,
                "category": cat,
                "status": "PASS" if passed else "FAIL",
                "caught": caught,
                "missed": missed,
                "falsePositives": fp,
                "codes": codes,
            }
            for name, cat, passed, caught, missed, fp, codes in rows
        ],
    }
    WEBSITE_REPORT.parent.mkdir(parents=True, exist_ok=True)
    WEBSITE_REPORT.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"  Website report: {WEBSITE_REPORT}")


def stamp_reference_docs(root: Path) -> None:
    """Refresh the score + commit quoted in the README/spec from the report.

    Best-effort and never fails the score; see
    `scripts/gen_conformance_reference.py` (also runnable standalone / in CI with
    `--check`).
    """
    script = root / "scripts" / "gen_conformance_reference.py"
    if not script.exists():
        return
    try:
        subprocess.run([sys.executable, str(script)], check=False)
    except OSError as exc:  # noqa: BLE001 — doc stamping must never break scoring
        print(f"  ⚠  could not stamp reference docs: {exc}")


# ---------------------------------------------------------------------------
# Run the binary with EVERY rule enabled — no disabling, ever
# ---------------------------------------------------------------------------


def purge_rule_config(conf_dir: Path) -> None:
    """Guarantee the binary scores with ALL rules enabled.

    Disabling any rule for conformance is forbidden (see module docstring). The
    only way `basilisk check <file>` could silence a rule is a `basilisk.json` it
    auto-discovers in the fixtures directory, so we DELETE any such file before
    scoring. With no config present the binary runs in its full, default,
    strict-by-default mode — every rule on — which is exactly what a real user
    gets. There is no config to write and no rule to turn off.
    """
    stale = conf_dir / "basilisk.json"
    if stale.exists():
        stale.unlink()


# ---------------------------------------------------------------------------
# Checker adapter — same role as upstream's per-checker adapters
# ---------------------------------------------------------------------------


class BasiliskTypeChecker:
    """Runs the real `basilisk` binary; parses its JSON into {line: [errors]}.

    Each diagnostic is the analog of the suite's `# E` ("an error MUST be
    reported on this line"). Both `error` and `warning` severities always count —
    the single, strictest grading, matching how the reference checker pyright is
    graded upstream. There is no looser mode.
    """

    name = "basilisk"

    def __init__(self, binary: Path) -> None:
        self.binary = binary

    def run_test(self, test_case: Path) -> str:
        proc = subprocess.run(
            [
                str(self.binary),
                "check",
                str(test_case),
                "--output",
                "json",
                "--color",
                "never",
            ],
            capture_output=True,
            text=True,
        )
        return proc.stdout

    def parse_errors(self, output: "Sequence[str] | str") -> dict[int, list[str]]:
        # upstream calls this with `output.splitlines()`; rejoin + parse JSON.
        text = "\n".join(output) if not isinstance(output, str) else output
        try:
            diags = json.loads(text) if text.strip() else []
        except json.JSONDecodeError:
            return {}
        accepted = {"error", "warning"}
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


def score(
    checker: "BasiliskTypeChecker",
    get_expected: Callable,
    diff_errors: Callable,
    conf_dir: Path,
) -> tuple[list[Path], list[Row], Totals]:
    # Match upstream's get_test_cases: score both `.py` and `.pyi` test files,
    # skip `_`-prefixed support modules (import-only inputs, no `# E` markers).
    # Real stub tests like `overloads_definitions_stub.pyi` MUST be graded.
    files = sorted(
        p
        for p in (*conf_dir.glob("*.py"), *conf_dir.glob("*.pyi"))
        if not p.name.startswith("_")
    )
    rows, totals = [], {"pass": 0, "missed": 0, "fp": 0, "caught": 0}
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
    return files, rows, totals


def print_scorecard(
    files: list[Path],
    rows: list[Row],
    totals: Totals,
    commit: dict,
    digest: str,
) -> None:
    n = len(files)
    pct = (totals["pass"] * 100.0 / n) if n else 0.0
    stale = "  [STALE — offline, cached fixtures]" if commit.get("stale") else ""
    print()
    print("=" * 68)
    print(
        "  BASILISK PEP CONFORMANCE — REAL python/typing CALCULATOR [errors+warnings]"
    )
    print("  calc: imported verbatim from vendored conformance/upstream_main.py")
    print(
        f"  ref:  python/typing@{commit.get('short', '?')}  ({digest})  "
        f"funcs: {', '.join(OFFICIAL_FUNCS)}{stale}"
    )
    print("=" * 68)
    print(f"  Files:    {n} total | {totals['pass']} pass | {n - totals['pass']} fail")
    print(f"  Score:    {pct:.1f}%   (Pass = empty errors_diff, upstream rule)")
    print(f"  Required: {totals['caught']} caught | {totals['missed']} missed")
    print(f"  False+:   {totals['fp']} unexpected diagnostics (THESE FAIL FILES)")
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
    # ONE grading, always: every diagnostic basilisk emits (errors AND warnings)
    # is counted as "an error was reported" — the strictest reading, and how the
    # reference checker pyright is graded upstream. There is no looser view and
    # no flag to weaken it; the scorer always does a full, strict generation.
    opts: dict = {
        "bin": None,
        "gate": False,
        "dir": None,
        "fetch_only": False,
    }
    it = iter(argv)
    for a in it:
        if a == "--bin":
            opts["bin"] = next(it, None)
        elif a == "--gate":
            opts["gate"] = True
        elif a == "--conformance-dir":
            opts["dir"] = next(it, None)
        elif a == "--fetch-only":
            opts["fetch_only"] = True
        # `--fetch` / `--refresh-upstream` are accepted for back-compat but are
        # no-ops: every run already tracks the latest `main`.
    return opts


def enforce_gate(root: Path, files: list[Path], totals: Totals) -> bool:
    n = len(files)
    pct = (totals["pass"] * 100) // n if n else 0
    threshold = read_conformance_field(root, "threshold")
    ceiling = read_conformance_field(root, "max_false_positives")
    failed = False
    if threshold is not None:
        if pct < threshold:
            print(
                f"  ✗ PEP conformance regression: {pct}% ({totals['pass']}/{n}) "
                f"< {threshold}% threshold.",
                file=sys.stderr,
            )
            failed = True
        else:
            print(
                f"  Conformance gate: {pct}% ({totals['pass']}/{n}) >= {threshold}% — PASS"
            )
    if ceiling is not None:
        if totals["fp"] > ceiling:
            print(
                f"  ✗ False-positive regression: {totals['fp']} FPs > {ceiling} ceiling.",
                file=sys.stderr,
            )
            failed = True
        else:
            print(f"  FP gate: {totals['fp']} <= {ceiling} ceiling — PASS")
    return not failed


def main(argv: list[str]) -> int:
    opts = parse_args(argv)
    root = repo_root()
    conf_dir = Path(opts["dir"]) if opts["dir"] else root / "conformance/tests"

    # Always track the latest python/typing@main; degrade to cached fixtures when
    # offline. Fatal only when there is no cache at all to fall back to.
    try:
        commit = fetch_upstream(conf_dir)
    except Exception as exc:  # noqa: BLE001 — surface fetch failure clearly
        print(f"  ✗ {exc}", file=sys.stderr)
        return 1
    if opts["fetch_only"]:
        return 0

    binary = find_binary(opts["bin"], root)
    if binary is None:
        print(
            "  ✗ basilisk binary not found. Build it or pass --bin <path>.",
            file=sys.stderr,
        )
        return 1

    try:
        get_expected, diff_errors, digest = load_official_calc()
    except Exception as exc:  # noqa: BLE001 — surface any load/verify failure clearly
        print(f"  ✗ could not load the official calculator: {exc}", file=sys.stderr)
        return 1

    # Run the binary with EVERY rule enabled. Disabling any rule for conformance
    # is forbidden — delete any stale config so nothing can silence a rule. The
    # honest number is what a real user gets out of the box. See [CHKARCH-CONFORMANCE].
    purge_rule_config(conf_dir)

    checker = BasiliskTypeChecker(binary)
    files, rows, totals = score(checker, get_expected, diff_errors, conf_dir)
    print_scorecard(files, rows, totals, commit, digest)
    write_csv(root, rows)
    write_website_report(commit, rows, totals, len(files))
    if not commit.get("stale"):
        stamp_reference_docs(root)

    if not opts["gate"]:
        return 0
    return 0 if enforce_gate(root, files, totals) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
