#!/usr/bin/env python3
# Implements [CHKARCH-CONFORMANCE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md
"""Score Basilisk by RUNNING the real python/typing conformance harness.

This is the ONE and ONLY conformance path. It reimplements NOTHING, vendors
NOTHING, and adapts NOTHING. Every run:

  1. clones ``python/typing@<ref>`` FRESH — the tests AND the harness — from the
     LATEST commit (no cache, no committed fixtures, no vendored calculator),
  2. runs the suite's OWN unmodified ``conformance/src/main.py --only-run
     basilisk`` against the ``--bin`` binary (via ``BASILISK_BIN``). The CI gate
     passes a freshly-built CLEAN release binary — exactly what ships, never the
     PyPI wheel (a prior version) and never an instrumented build.

The suite already ships the official Basilisk adapter — ``BasiliskTypeChecker``
in ``conformance/src/type_checker.py``
(https://github.com/python/typing/blob/main/conformance/src/type_checker.py) —
so there is nothing of ours to inject. The harness writes
``results/basilisk/*.toml``; every ``Pass``/``Fail`` verdict and every
``errors_diff`` in those files is the harness's own, computed by the same code
that grades pyright, mypy, pyrefly, ty, zuban and pycroscope.

From those REAL results this script only *reports* — it never re-scores:

  * gates the score (delegates to ``assert_wheel_conformance.py``: 100 %, 0 FP),
  * writes ``conformance/conformance_status.csv`` (per-file pass/fail + stats),
  * writes ``website/src/_data/conformance_report.json`` (resolved commit +
    score) for the website + doc stamps,
  * mirrors the graded fixtures into ``conformance/tests/`` (some Rust tests
    read them),
  * stamps the score/commit into the README + spec.

The only per-file number not written to the toml by the harness is ``caught``
(required errors matched — the counterpart to ``missed``). It is taken from
upstream's OWN ``get_expected_errors``, imported live from the fresh clone — the
official function on the official tests, never a copy.

⚠️ There is NO cached-fixtures fallback and NO vendored calculator, by design.
If the real suite cannot be cloned and run, this FAILS — a build in which the
official check did not run is a BUILD FAILURE. Disabling any rule, hand-editing
the CSV, or loosening the gate to fake a pass is forbidden: close every gap by
FIXING the Rust checker. See [CHKARCH-CONFORMANCE].

Usage:
    python3 conformance/run_conformance.py [--bin PATH] [--gate]
                                           [--ref REF] [--suite-dir DIR]
                                           [--reuse-clone] [--sync-tests]
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Callable

from worktree_lock import (
    WorktreeBusyError,
    exclusive_worktree_lock,
    inherited_lock_is_valid,
)

# The suite's own harness imports these at runtime; the score/`caught` pass also
# imports its calculator, so this interpreter must have them. NOT a reimplement.
HARNESS_DEPS = ("jinja2", "markdown", "tomlkit")
UPSTREAM_REPO = "python/typing"
UPSTREAM_URL = f"https://github.com/{UPSTREAM_REPO}"
UPSTREAM_REF = "main"
RUN_ID_ENV = "BASILISK_CONFORMANCE_RUN_ID"
# The two functions that ARE the official scoring algorithm; we reuse only the
# first (for the `caught` stat). The harness itself applies both to grade files.
OFFICIAL_FUNCS = ("get_expected_errors", "diff_expected_errors")
Row = tuple[str, str, bool, int, int, int, list[str]]


def repo_root() -> Path:
    here = Path(__file__).resolve()
    for parent in here.parents:
        if (parent / "Cargo.toml").exists() and (parent / "crates").exists():
            return parent
    return here.parent.parent


def run(cmd: list[str], *, cwd: Path | None = None, env: dict | None = None) -> None:
    print("+", " ".join(cmd))
    subprocess.run(cmd, cwd=cwd, env=env, check=True)


def _is_current_venv(venv: Path, *, prefix: str | None = None) -> bool:
    """Return whether this interpreter is running inside ``venv``.

    Comparing resolved executable paths is incorrect: virtualenv launchers are
    commonly symlinks to the system interpreter, so both paths resolve to the
    same file even before the virtualenv has been entered.
    """
    return Path(prefix or sys.prefix).resolve() == venv.resolve()


def ensure_harness_deps(root: Path) -> None:
    """Guarantee the real harness's runtime deps are importable.

    When they already are (CI pre-installs them; see ci.yml), this returns at
    once. Otherwise it provisions a git-ignored venv under ``target/`` and
    re-execs into it, so ``make test`` / ``make conformance`` run the real suite
    with no manual pip step. This installs the harness's OWN dependencies — it
    changes nothing about how files are scored.
    """
    try:
        for dep in HARNESS_DEPS:
            importlib.import_module(dep)
        return
    except ImportError:
        pass

    venv = root / "target" / ".conformance-venv"
    py = venv / ("Scripts" if os.name == "nt" else "bin") / "python"
    if not py.exists():
        run([sys.executable, "-m", "venv", str(venv)])
        run([str(py), "-m", "pip", "install", "-q", "--upgrade", "pip"])

    # A cached virtualenv may exist without all dependencies (for example, an
    # interrupted install). Check it every time instead of assuming that the
    # interpreter's existence proves the environment is complete.
    dependency_probe = [
        str(py),
        "-c",
        "; ".join(f"import {dep}" for dep in HARNESS_DEPS),
    ]
    if subprocess.run(dependency_probe, check=False).returncode != 0:
        run([str(py), "-m", "pip", "install", "-q", *HARNESS_DEPS])

    if _is_current_venv(venv):
        importlib.invalidate_caches()
        try:
            for dep in HARNESS_DEPS:
                importlib.import_module(dep)
            return
        except ImportError as exc:
            raise RuntimeError(
                f"harness deps {HARNESS_DEPS} missing even inside {venv}; "
                "provisioning failed"
            ) from exc
    os.execv(str(py), [str(py), str(Path(__file__).resolve()), *sys.argv[1:]])


# ---------------------------------------------------------------------------
# Clone the REAL suite fresh (no cache) and run its OWN harness
# ---------------------------------------------------------------------------


def clone_suite(ref: str, dest: Path) -> tuple[Path, dict]:
    """Clone ``python/typing@ref`` FRESH into ``dest`` and resolve its commit."""
    if shutil.which("git") is None:
        raise RuntimeError("git not found on PATH — cannot clone the real suite")
    if dest.exists():
        raise RuntimeError(f"fresh clone destination must not already exist: {dest}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    args = ["git", "clone", "--depth", "1"]
    if ref != UPSTREAM_REF:
        args += ["--branch", ref]
    run(args + [UPSTREAM_URL, str(dest)])
    suite = _suite_paths(dest)
    run_id = os.environ.get(RUN_ID_ENV)
    if run_id:
        _owner_marker(dest).write_text(run_id, encoding="utf-8")
    return suite


def _git_out(dest: Path, *args: str) -> str:
    """``git -C dest <args>`` stdout, stripped."""
    return subprocess.run(
        ["git", "-C", str(dest), *args],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def _suite_paths(dest: Path) -> tuple[Path, dict]:
    conf = dest / "conformance"
    if not (conf / "src" / "main.py").is_file():
        raise RuntimeError(f"not a python/typing checkout: missing {conf}/src/main.py")
    sha = _git_out(dest, "rev-parse", "HEAD")
    date = _git_out(dest, "log", "-1", "--format=%cs")
    return conf, {"sha": sha, "short": sha[:7], "date": date}


def run_harness(conf_dir: Path, binary: Path) -> Path:
    """Run the suite's own ``src/main.py --only-run basilisk`` on ``binary``.

    ``BASILISK_BIN`` points the upstream ``BasiliskTypeChecker`` at our compiled
    binary. Returns the ``results/basilisk`` directory the harness wrote.
    """
    env = {**os.environ, "BASILISK_BIN": str(binary.resolve())}
    run(
        [sys.executable, "src/main.py", "--only-run", "basilisk"],
        cwd=conf_dir,
        env=env,
    )
    results = conf_dir / "results" / "basilisk"
    tomls = [p for p in results.glob("*.toml") if p.name != "version.toml"]
    if not tomls:
        raise RuntimeError(
            f"the real harness wrote no results to {results} — it did not run"
        )
    return results


def load_get_expected(conf_dir: Path) -> Callable:
    """Import upstream's REAL ``get_expected_errors`` from the fresh clone.

    Used ONLY to count ``caught`` (required errors matched). It is the official
    function operating on the official tests — imported, never copied.
    """
    src = conf_dir / "src"
    if str(src) not in sys.path:
        sys.path.insert(0, str(src))
    spec = importlib.util.spec_from_file_location(
        "typing_conformance_main", src / "main.py"
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import the upstream calculator from {src}/main.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    func = getattr(module, OFFICIAL_FUNCS[0], None)
    if func is None:
        raise RuntimeError(
            f"upstream main.py is missing {OFFICIAL_FUNCS[0]} — the layout changed"
        )
    return func


# ---------------------------------------------------------------------------
# Turn the harness's OWN results into rows — no re-scoring
# ---------------------------------------------------------------------------


def category(name: str) -> str:
    stem = name.lstrip("_")
    return stem.split("_", 1)[0] if "_" in stem else stem[:-3]


def _emitted_and_codes(output: str) -> tuple[set[int], list[str]]:
    """Distinct emitted line numbers + sorted diagnostic codes from toml output.

    Each output line is ``file:line:col: severity: message [code]`` — exactly the
    text the upstream adapter produced. We only read it back.
    """
    lines, codes = set(), set()
    for raw in output.splitlines():
        parts = raw.split(":", 3)
        if len(parts) >= 3 and parts[1].strip().isdigit():
            lines.add(int(parts[1]))
        stripped = raw.rstrip()
        if stripped.endswith("]") and "[" in stripped:
            codes.add(stripped[stripped.rfind("[") + 1 : -1])
    return lines, sorted(codes)


def build_rows(
    results: Path, tests: Path, get_expected: Callable
) -> tuple[list[Row], dict]:
    import tomllib

    rows: list[Row] = []
    totals = {"pass": 0, "missed": 0, "fp": 0, "caught": 0}
    for toml_path in sorted(results.glob("*.toml")):
        if toml_path.name == "version.toml":
            continue
        data = tomllib.loads(toml_path.read_text(encoding="utf-8"))
        diff = (data.get("errors_diff") or "").strip()
        emitted, codes = _emitted_and_codes(data.get("output") or "")

        stem = toml_path.stem
        fixture = tests / f"{stem}.py"
        if not fixture.exists() and (tests / f"{stem}.pyi").exists():
            fixture = tests / f"{stem}.pyi"

        passed = data.get("conformance_automated") == "Pass" and not diff
        missed = sum(1 for ln in diff.splitlines() if "Expected" in ln)
        fp = sum(1 for ln in diff.splitlines() if "Unexpected" in ln)

        # `caught` = required-error lines (req>0) the binary emitted on, via the
        # OFFICIAL get_expected_errors. At 0 missed this equals the total required.
        expected, _optional = get_expected(fixture)
        required = [ln for ln, (req, _o) in expected.items() if req > 0]
        caught = sum(1 for ln in required if ln in emitted)

        rows.append(
            (fixture.name, category(fixture.name), passed, caught, missed, fp, codes)
        )
        totals["pass"] += int(passed)
        totals["missed"] += missed
        totals["fp"] += fp
        totals["caught"] += caught
    return rows, totals


# ---------------------------------------------------------------------------
# Emit the committed artefacts (CSV + website report) + mirror fixtures
# ---------------------------------------------------------------------------


def write_csv(root: Path, rows: list[Row]) -> None:
    lines = ["basilisk_rules,file,category,status,caught,missed,false_positives"]
    for name, cat, passed, caught, missed, fp, codes in rows:
        status = "PASS" if passed else "FAIL"
        lines.append(f"{'|'.join(codes)},{name},{cat},{status},{caught},{missed},{fp}")
    out = root / "conformance" / "conformance_status.csv"
    out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"  Conformance CSV: {out}")


def write_report(
    root: Path, commit: dict, rows: list[Row], totals: dict, calc: dict
) -> None:
    n = len(rows)
    pct = round(totals["pass"] * 100.0 / n, 1) if n else 0.0
    report = {
        "_doc": (
            "Generated by conformance/run_conformance.py on every run from the REAL "
            "python/typing harness output. The website build "
            "(website/src/_data/conformance.js) reads this for the upstream commit. "
            "Do not hand-edit."
        ),
        "upstream": {
            "repo": UPSTREAM_REPO,
            "ref": UPSTREAM_REF,
            "sha": commit["sha"],
            "shortSha": commit["short"],
            "commitDate": commit["date"],
            "stale": False,
        },
        "calculator": {
            "file": f"{UPSTREAM_REPO}@{commit['short']}:conformance/src/main.py",
            "sha256": calc["sha256"],
            "bytes": calc["bytes"],
            "funcs": list(OFFICIAL_FUNCS),
        },
        "grading": "real python/typing harness (src/main.py --only-run basilisk), every rule enabled",
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
    out = root / "website" / "src" / "_data" / "conformance_report.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"  Website report: {out}")


def mirror_tests(conf_dir: Path, root: Path) -> None:
    """Copy the graded fixtures into ``conformance/tests`` for the Rust tests."""
    src = conf_dir / "tests"
    dst = root / "conformance" / "tests"
    dst.mkdir(parents=True, exist_ok=True)
    for stale in (*dst.glob("*.py"), *dst.glob("*.pyi")):
        stale.unlink()
    copied = 0
    for fixture in (*src.glob("*.py"), *src.glob("*.pyi")):
        shutil.copy2(fixture, dst / fixture.name)
        copied += 1
    print(f"  Mirrored {copied} fixtures -> {dst}")


def stamp_docs(root: Path) -> None:
    script = root / "scripts" / "gen_conformance_reference.py"
    if script.exists():
        subprocess.run([sys.executable, str(script)], check=False)


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def find_binary(explicit: str | None, root: Path) -> Path | None:
    if explicit:
        p = Path(explicit)
        return p if p.exists() else None
    for candidate in (
        root / "target/ci/basilisk",
        root / "target/release/basilisk",
        root / "target/debug/basilisk",
    ):
        if candidate.exists():
            return candidate
    return None


def parse_args(argv: list[str]) -> dict:
    opts = {
        "bin": None,
        "gate": False,
        "ref": UPSTREAM_REF,
        "suite_dir": None,
        "reuse_clone": False,
        "sync_tests": False,
    }
    it = iter(argv)
    for arg in it:
        if arg == "--bin":
            opts["bin"] = next(it, None)
        elif arg == "--gate":
            opts["gate"] = True
        elif arg == "--ref":
            opts["ref"] = next(it, None) or UPSTREAM_REF
        elif arg == "--suite-dir":
            opts["suite_dir"] = next(it, None)
        elif arg == "--reuse-clone":
            opts["reuse_clone"] = True
        elif arg == "--sync-tests":
            opts["sync_tests"] = True
    return opts


def _ensure_outside_repo(destination: Path, root: Path) -> None:
    """Reject suite paths that can inherit this repository's configuration."""
    resolved = destination.resolve()
    repository = root.resolve()
    if resolved == repository or repository in resolved.parents:
        raise RuntimeError(
            "conformance suite must remain outside the Basilisk repository"
        )


def _owner_marker(destination: Path) -> Path:
    """Return the run-ownership marker outside the pristine checkout tree."""
    return destination.parent / f".{destination.name}.basilisk-owner"


def _suite_destination(
    opts: dict, root: Path
) -> tuple[Path, tempfile.TemporaryDirectory | None]:
    """Return a caller-owned path or an isolated invocation-owned path."""
    if opts["suite_dir"]:
        destination = Path(opts["suite_dir"])
        _ensure_outside_repo(destination, root)
        return destination, None
    if opts["reuse_clone"]:
        raise RuntimeError(
            "--reuse-clone requires an explicit --suite-dir owned by this run"
        )
    owner = tempfile.TemporaryDirectory(prefix="basilisk-typing-upstream-")
    destination = Path(owner.name) / "typing"
    try:
        _ensure_outside_repo(destination, root)
    except RuntimeError:
        owner.cleanup()
        raise
    return destination, owner


def resolve_suite(opts: dict, dest: Path) -> tuple[Path, dict]:
    # The clone lives OUTSIDE the repository tree: per-file config discovery
    # walks ancestor directories ([CHKARCH-CONFIG-DISCOVERY]), so a clone under
    # the repo would inherit the repo's own `[tool.basilisk]` rules and the
    # score would no longer be the binary's out-of-the-box default. A neutral
    # system-temp location keeps the suite config-free, exactly what a user
    # gets with no configuration ([CHKARCH-CONFORMANCE]).
    if opts["reuse_clone"]:
        run_id = os.environ.get(RUN_ID_ENV)
        marker = _owner_marker(dest)
        if (
            not run_id
            or not marker.is_file()
            or marker.read_text(encoding="utf-8") != run_id
        ):
            raise RuntimeError(
                "--reuse-clone requires a checkout owned by the active conformance run"
            )
        conf, commit = _suite_paths(dest)
        print(f"  Reusing fresh clone at {dest} (python/typing@{commit['short']})")
        return conf, commit
    return clone_suite(opts["ref"], dest)


def assert_graded_commit_is_live_main(commit: dict) -> None:
    """FAIL the gate unless the graded suite IS the live ``python/typing@main`` tip.

    The gate exists to catch the moment upstream adds a test we do not pass, so
    grading anything other than the CURRENT tip silently defeats it. A stale
    tree can reach the harness three ways — ``--ref`` naming another branch/tag,
    ``--suite-dir`` pointing at a checkout from an earlier run, or
    ``--reuse-clone`` re-entering one — and all three would otherwise score
    100% against yesterday's tests and report a pass.

    So in gate mode the graded HEAD is compared against ``git ls-remote`` for
    ``main``, live. A mismatch, an unreachable remote, or a missing ref all
    FAIL: an unverifiable score is not a passing score ([CHKARCH-CONFORMANCE]).
    """
    try:
        out = subprocess.run(
            ["git", "ls-remote", UPSTREAM_URL, f"refs/heads/{UPSTREAM_REF}"],
            check=True,
            capture_output=True,
            text=True,
            timeout=120,
        ).stdout.strip()
    except (subprocess.CalledProcessError, OSError, subprocess.TimeoutExpired) as exc:
        raise RuntimeError(
            f"gate cannot verify the graded commit against {UPSTREAM_REPO}@"
            f"{UPSTREAM_REF}: {exc}. The conformance gate refuses to pass a score "
            "it cannot prove was measured against the current suite."
        ) from exc
    live = out.split("\t", 1)[0].strip() if out else ""
    if not live:
        raise RuntimeError(
            f"gate could not resolve {UPSTREAM_REPO}@{UPSTREAM_REF} — refusing to "
            "grade against an unverifiable suite."
        )
    if live != commit["sha"]:
        raise RuntimeError(
            "STALE CONFORMANCE SUITE — the gate graded "
            f"{commit['short']} ({commit['date']}) but {UPSTREAM_REPO}@"
            f"{UPSTREAM_REF} is now {live[:7]}. Every gate run must score the "
            "CURRENT suite, or a newly added upstream test can never fail us. "
            "Re-run without --suite-dir/--reuse-clone/--ref so the suite is "
            "cloned fresh."
        )
    print(
        f"  gate suite verified: {UPSTREAM_REPO}@{commit['short']} is {UPSTREAM_REF} tip"
    )


def _run_with_suite(opts: dict, root: Path, suite_dir: Path) -> int:
    """Run one phase against the suite directory owned by the caller."""
    conf_dir, commit = resolve_suite(opts, suite_dir)

    # --sync-tests: only refresh the fixtures the Rust suite reads, then stop.
    # (Runs before `cargo test`, which needs conformance/tests present.)
    if opts["sync_tests"]:
        mirror_tests(conf_dir, root)
        return 0

    binary = find_binary(opts["bin"], root)
    if binary is None:
        print(
            "  ✗ basilisk binary not found. Build it or pass --bin <path>.",
            file=sys.stderr,
        )
        return 1

    results = run_harness(conf_dir, binary)
    mirror_tests(conf_dir, root)

    get_expected = load_get_expected(conf_dir)
    rows, totals = build_rows(results, conf_dir / "tests", get_expected)

    main_py = conf_dir / "src" / "main.py"
    raw = main_py.read_bytes()
    calc = {"sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}

    write_csv(root, rows)
    write_report(root, commit, rows, totals, calc)
    stamp_docs(root)

    if not opts["gate"]:
        return 0

    # The score only means something if it was measured against the CURRENT
    # suite — verify that before trusting it ([CHKARCH-CONFORMANCE]).
    assert_graded_commit_is_live_main(commit)

    # The gate is the kept assert_wheel_conformance.py, run over the harness's OWN
    # results (100% pass, 0 false positives, from coverage-thresholds.json). It
    # reads the real *.toml — no scoring of ours.
    gate = subprocess.run(
        [
            sys.executable,
            str(root / "conformance" / "assert_wheel_conformance.py"),
            str(results),
        ]
    )
    return gate.returncode


def _run_owned(opts: dict, root: Path) -> int:
    """Run after the caller has established exclusive worktree ownership."""
    suite_dir, owner = _suite_destination(opts, root)
    try:
        return _run_with_suite(opts, root, suite_dir)
    finally:
        if owner is not None:
            owner.cleanup()


def main(argv: list[str]) -> int:
    opts = parse_args(argv)
    root = repo_root()
    lock_path = root / "target" / "test-rust.lock"
    if inherited_lock_is_valid(lock_path):
        ensure_harness_deps(root)
        return _run_owned(opts, root)
    try:
        with exclusive_worktree_lock(lock_path):
            ensure_harness_deps(root)
            return _run_owned(opts, root)
    except WorktreeBusyError as exc:
        print(f"  ✗ {exc}", file=sys.stderr)
        return 75


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
