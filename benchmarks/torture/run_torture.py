#!/usr/bin/env python3
"""Type-torture scoreboard: Basilisk vs pyright, mypy, ty, pyrefly, zuban.

Implements the first slice of [NARROWPLAN-SCOREBOARD] — see
docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-SCOREBOARD.

Eight small, hard typing problems (benchmarks/torture/cases/*.py), each
grounded in a typing-spec section, an accepted PEP, or Python language
semantics — several are reproducers from this repo's own issue tracker
(#371 recursive aliases, #398 recursive-base hang, #374 enum literal
expansion, #317 gradual unannotated code, #284 tuple-index false positive).

METHODOLOGY (stated here because the results are published):

  * Every tool runs in its OUT-OF-THE-BOX default configuration on a
    config-neutral copy of each case — the same "what a user gets with no
    config" frame as the upstream python/typing conformance harness and
    benchmarks/run.sh. No strictness flags for anyone.
  * Scoring is conformance-style and exact, per case: a line whose source
    ends in `# E` REQUIRES at least one error diagnostic on that line; a
    line without the marker must have NONE. A tool passes a case iff both
    hold. Error severity only — warnings, notes, and infos never count.
  * A tool that exceeds the per-invocation timeout is scored `hang` (the
    #398 axis: termination is part of correctness). A tool that exits >= 2
    with no parseable diagnostics is scored `crash`. Both fail the case.
  * Competitor versions are the LATEST official release, pulled (best
    effort, loudly on failure) at the top of every run — leads are proven
    against current upstream, never a stale pin.
  * WHICH basilisk binary was measured is recorded in both published
    artifacts: path, local-build-vs-installed-artifact, and (for a local
    build) the `git describe` of the tree it came from. A working-tree build
    and the artifact a user installs can disagree — on `recursive_bases.py`
    they did, the tree passing while the shipped binary hung — so a
    scoreboard that does not name its binary cannot be audited. See
    docs/specs/TYPE-TORTURE-HANG-INCIDENT.md#TORTURE-HANG-GAP. Point
    BASILISK_BIN at an installed artifact to score that instead.

WRITE-ALWAYS, GATE-SEPARATELY (same contract as benchmarks/summarize.py):

  1. WRITE. The scoreboard CSV (benchmarks/torture/status/torture.csv) is
     rewritten from the accumulated results after EVERY case completes.
     There is no gate on the write: the file always shows exactly what this
     run measured, the instant each verdict exists.
  2. GATE. After all cases, the run's basilisk verdicts are compared
     against the COMMITTED CSV (read from git at HEAD, never the working
     copy just overwritten). A case basilisk passed at HEAD that no longer
     passes exits 3 -> CI failure. The gate only reads; it never edits.
     With no committed CSV yet, the baseline establishes on first commit.

Usage:  python3 benchmarks/torture/run_torture.py
Knobs:  TORTURE_TIMEOUT (seconds per invocation, default 30)
        TORTURE_NO_PULL=1 (skip the competitor pull: local iteration only,
        refused in CI so published columns always reflect latest upstream)
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CASES_DIR = Path(__file__).resolve().parent / "cases"
STATUS_CSV = Path(__file__).resolve().parent / "status" / "torture.csv"
SUMMARY_MD = Path(__file__).resolve().parent / "results" / "summary.md"
COMPETITORS = ["pyright", "mypy", "ty", "pyrefly", "zuban"]
TIMEOUT = int(os.environ.get("TORTURE_TIMEOUT", "30"))


@dataclass
class Outcome:
    """One tool's verdict on one case."""

    verdict: str  # pass | fail | hang | crash
    missed: list[int]
    extra: list[int]

    def cell(self) -> str:
        if self.verdict != "fail":
            return self.verdict
        return f"fail(m{len(self.missed)},x{len(self.extra)})"


def fail_usage(message: str) -> "sys.NoReturn":
    print(f"ERROR: {message}", file=sys.stderr)
    sys.exit(2)


def basilisk_bin() -> Path:
    binary = Path(
        os.environ.get("BASILISK_BIN", ROOT / "target" / "release" / "basilisk")
    )
    if not binary.is_file():
        fail_usage(
            f"basilisk binary not found at {binary} — build with `cargo build --release`."
        )
    return binary


def source_revision() -> str:
    """Short revision of the tree being measured, with a `-dirty` marker.

    The incident that motivated this recorded a shipped artifact built from a
    commit contained in no tag, so `--tags` is deliberate: it exposes exactly
    that skew ([TORTURE-HANG-PROVENANCE]).
    """
    result = subprocess.run(
        ["git", "-C", str(ROOT), "describe", "--always", "--dirty", "--tags"],
        capture_output=True,
        text=True,
        check=False,
    )
    return result.stdout.strip() or "unknown" if result.returncode == 0 else "unknown"


def basilisk_provenance(binary: Path) -> str:
    """How the measured basilisk binary was obtained — [TORTURE-HANG-GAP].

    A published scoreboard that does not say WHICH binary produced its numbers
    cannot be audited. A working-tree build and the artifact a user installs
    can disagree, and on `recursive_bases.py` they did: the tree passed in
    0.06 s while the installed binary hung
    (docs/specs/TYPE-TORTURE-HANG-INCIDENT.md#TORTURE-HANG-PROVENANCE).
    """
    resolved = binary.resolve()
    in_tree = resolved.is_relative_to((ROOT / "target").resolve())
    kind = "local working-tree build" if in_tree else "installed artifact"
    overridden = " via BASILISK_BIN" if "BASILISK_BIN" in os.environ else ""
    source = f", built from {source_revision()}" if in_tree else ""
    return f"{kind}{overridden}: {resolved}{source}"


def pull_latest() -> None:
    """Best-effort upgrade of every competitor to its newest official release.

    Mirrors benchmarks/run.sh: a failed pull warns LOUDLY and the run
    continues on the installed version — visible in the log, never silent.
    """
    if os.environ.get("TORTURE_NO_PULL"):
        if os.environ.get("GITHUB_ACTIONS") == "true":
            fail_usage(
                "TORTURE_NO_PULL is a local iteration mode; CI must pull latest."
            )
        print("  local iteration mode — competitors NOT pulled; columns may be stale.")
        return
    for tool in COMPETITORS:
        result = subprocess.run(
            [
                sys.executable,
                "-m",
                "pip",
                "install",
                "--upgrade",
                "--quiet",
                "--disable-pip-version-check",
                tool,
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            print(
                f"  ⚠ {tool}: could not pull latest — using installed version. "
                f"Column may be stale.",
                file=sys.stderr,
            )


def tool_version(name: str, command: list[str]) -> str:
    try:
        result = subprocess.run(
            [command[0], "--version"],
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
        first_line = (result.stdout or result.stderr).strip().splitlines()
        return first_line[0] if first_line else "unknown"
    except (OSError, subprocess.TimeoutExpired):
        return "not installed"


def expected_error_lines(case: Path) -> set[int]:
    """Line numbers (1-based) whose source line ends with the `# E` marker."""
    lines = case.read_text(encoding="utf-8").splitlines()
    return {
        index
        for index, line in enumerate(lines, start=1)
        if line.rstrip().endswith("# E")
    }


def parse_mypy_style(output: str, filename: str) -> set[int]:
    """`path:LINE: error: ...` lines (mypy and zuban)."""
    reported: set[int] = set()
    for line in output.splitlines():
        parts = line.split(":", 3)
        if len(parts) >= 3 and Path(parts[0]).name == filename:
            line_number, severity = parts[1].strip(), parts[2].strip()
            if line_number.isdigit() and severity == "error":
                reported.add(int(line_number))
    return reported


def parse_pyright_json(output: str, filename: str) -> set[int]:
    """pyright --outputjson: generalDiagnostics with severity == error."""
    try:
        payload = json.loads(output)
    except json.JSONDecodeError:
        return set()
    reported: set[int] = set()
    for diagnostic in payload.get("generalDiagnostics", []):
        if diagnostic.get("severity") != "error":
            continue
        if Path(diagnostic.get("file", "")).name != filename:
            continue
        line = diagnostic.get("range", {}).get("start", {}).get("line")
        if isinstance(line, int):
            reported.add(line + 1)  # pyright ranges are 0-based
    return reported


def parse_arrow_style(
    output: str, filename: str, error_prefix: str, demote_prefix: str
) -> set[int]:
    """Header + `--> path:line:col` blocks (basilisk, ty, pyrefly).

    An error header arms attribution; the next `-->` location consumes it.
    A warning header disarms it so a warning's location is never counted.
    """
    reported: set[int] = set()
    armed = False
    for line in output.splitlines():
        stripped = line.strip()
        if stripped.startswith(error_prefix):
            armed = True
            continue
        if stripped.startswith(demote_prefix):
            armed = False
            continue
        if armed and stripped.startswith("-->"):
            location = stripped.removeprefix("-->").strip()
            parts = location.split(":")
            if (
                len(parts) >= 2
                and Path(parts[0]).name == filename
                and parts[1].isdigit()
            ):
                reported.add(int(parts[1]))
            armed = False
    return reported


def tool_commands(basilisk: Path, mypy_cache: Path) -> list[tuple[str, list[str]]]:
    """(name, argv-with-{file}-placeholder) per tool, defaults only."""
    return [
        ("basilisk", [str(basilisk), "check", "{file}"]),
        ("pyright", ["pyright", "--outputjson", "{file}"]),
        (
            "mypy",
            [
                "mypy",
                "--no-incremental",
                "--no-error-summary",
                "--cache-dir",
                str(mypy_cache),
                "{file}",
            ],
        ),
        ("ty", ["ty", "check", "{file}"]),
        ("pyrefly", ["pyrefly", "check", "{file}"]),
        ("zuban", ["zuban", "check", "{file}"]),
    ]


def parse_output(tool: str, output: str, filename: str) -> set[int]:
    if tool == "pyright":
        return parse_pyright_json(output, filename)
    if tool in ("mypy", "zuban"):
        return parse_mypy_style(output, filename)
    if tool == "pyrefly":
        return parse_arrow_style(output, filename, "ERROR", "WARN")
    return parse_arrow_style(output, filename, "error[", "warning[")


def run_case(tool: str, argv: list[str], case: Path, workdir: Path) -> Outcome:
    command = [part.replace("{file}", case.name) for part in argv]
    try:
        result = subprocess.run(
            command,
            cwd=workdir,
            capture_output=True,
            text=True,
            timeout=TIMEOUT,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return Outcome("hang", [], [])
    except OSError:
        return Outcome("crash", [], [])
    reported = parse_output(tool, result.stdout + "\n" + result.stderr, case.name)
    if result.returncode >= 2 and not reported and tool != "pyright":
        return Outcome("crash", [], [])
    expected = expected_error_lines(case)
    missed = sorted(expected - reported)
    extra = sorted(reported - expected)
    if not missed and not extra:
        return Outcome("pass", [], [])
    return Outcome("fail", missed, extra)


def write_status(
    tools: list[str],
    versions: dict[str, str],
    results: dict[str, dict[str, Outcome]],
    provenance: str,
) -> None:
    """WRITE-ALWAYS: rewrite the tracked CSV from every verdict so far."""
    STATUS_CSV.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "# Type-torture scoreboard — see benchmarks/torture/run_torture.py for the",
        "# full methodology. Self-measured: every tool in its out-of-the-box default",
        "# config, same machine, same corpus; scored conformance-style (`# E` lines",
        "# require an error; unmarked lines require silence; error severity only).",
        "# hang = exceeded the per-invocation timeout; crash = exit >= 2 with no",
        "# parseable diagnostics. Regenerated by every run; never hand-edited.",
        f"# measured binary: {provenance}",
    ]
    lines.extend(f"# {tool}: {versions[tool]}" for tool in tools)
    lines.append("case," + ",".join(tools))
    for case_name in sorted(results):
        cells = [results[case_name][tool].cell() for tool in tools]
        lines.append(f"{case_name}," + ",".join(cells))
    if results:
        totals = [
            str(sum(1 for case in results.values() if case[tool].verdict == "pass"))
            + f"/{len(results)}"
            for tool in tools
        ]
        lines.append("passed," + ",".join(totals))
    STATUS_CSV.write_text("\n".join(lines) + "\n", encoding="utf-8")


def write_summary(
    tools: list[str],
    versions: dict[str, str],
    results: dict[str, dict[str, Outcome]],
    provenance: str,
) -> None:
    lines = [
        "# Type-torture results",
        "",
        "Methodology: see the header of `benchmarks/torture/run_torture.py` and of",
        "`benchmarks/torture/status/torture.csv`. Every case file states the spec",
        "section or PEP that makes its expectations authoritative.",
        "",
        f"Measured basilisk binary: {provenance}",
        "",
        "| case | " + " | ".join(tools) + " |",
        "|---" * (len(tools) + 1) + "|",
    ]
    for case_name in sorted(results):
        row = [results[case_name][tool].cell() for tool in tools]
        lines.append(f"| {case_name} | " + " | ".join(row) + " |")
    totals = [
        str(sum(1 for case in results.values() if case[tool].verdict == "pass"))
        + f"/{len(results)}"
        for tool in tools
    ]
    lines.append("| **passed** | " + " | ".join(totals) + " |")
    lines.extend(
        ["", "Versions measured: " + "; ".join(f"{t} {versions[t]}" for t in tools), ""]
    )
    for case_name in sorted(results):
        details = [
            f"- {tool}: missed error lines {outcome.missed}, false positives on {outcome.extra}"
            for tool, outcome in results[case_name].items()
            if outcome.verdict == "fail"
        ]
        hangs = [
            f"- {tool}: {outcome.verdict}"
            for tool, outcome in results[case_name].items()
            if outcome.verdict in ("hang", "crash")
        ]
        if details or hangs:
            lines.extend([f"## {case_name}", *details, *hangs, ""])
    SUMMARY_MD.parent.mkdir(parents=True, exist_ok=True)
    SUMMARY_MD.write_text("\n".join(lines) + "\n", encoding="utf-8")


def committed_basilisk_passes() -> set[str] | None:
    """Case names basilisk passes in the COMMITTED CSV (None = no baseline)."""
    relative = STATUS_CSV.relative_to(ROOT)
    result = subprocess.run(
        ["git", "-C", str(ROOT), "show", f"HEAD:{relative}"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        return None
    passes: set[str] = set()
    header: list[str] = []
    for line in result.stdout.splitlines():
        if line.startswith("#") or not line.strip():
            continue
        cells = line.split(",")
        if cells[0] == "case":
            header = cells
            continue
        if cells[0] == "passed" or "basilisk" not in header:
            continue
        if cells[header.index("basilisk")] == "pass":
            passes.add(cells[0])
    return passes


def gate(results: dict[str, dict[str, Outcome]]) -> None:
    """GATE-SEPARATELY: basilisk may never lose a case it passed at HEAD."""
    baseline = committed_basilisk_passes()
    if baseline is None:
        print(
            "  no committed baseline yet — it establishes when this CSV is committed."
        )
        return
    regressions = [
        case
        for case in sorted(baseline)
        if case in results and results[case]["basilisk"].verdict != "pass"
    ]
    if regressions:
        print(
            f"GATE FAILURE: basilisk regressed on: {', '.join(regressions)}",
            file=sys.stderr,
        )
        sys.exit(3)
    print("  gate: no basilisk regression against the committed baseline.")


def main() -> None:
    cases = sorted(CASES_DIR.glob("*.py"))
    if not cases:
        fail_usage(f"no cases found in {CASES_DIR}")
    print("Pulling latest competitor releases (best effort)…")
    pull_latest()
    with tempfile.TemporaryDirectory(prefix="basilisk-torture.") as tmp:
        workdir = Path(tmp)
        for case in cases:
            shutil.copy(case, workdir / case.name)
        binary = basilisk_bin()
        provenance = basilisk_provenance(binary)
        print(f"Measuring basilisk from {provenance}")
        tools = tool_commands(binary, workdir / ".mypy_cache_torture")
        names = [name for name, _ in tools]
        versions = {name: tool_version(name, argv) for name, argv in tools}
        results: dict[str, dict[str, Outcome]] = {}
        for case in cases:
            results[case.stem] = {
                name: run_case(name, argv, case, workdir) for name, argv in tools
            }
            # write-always, per case
            write_status(names, versions, results, provenance)
            cells = ", ".join(f"{n}={results[case.stem][n].cell()}" for n in names)
            print(f"  {case.stem}: {cells}")
    write_summary(names, versions, results, provenance)
    print(f"Scoreboard: {STATUS_CSV}\nSummary:    {SUMMARY_MD}")
    gate(results)


if __name__ == "__main__":
    main()
