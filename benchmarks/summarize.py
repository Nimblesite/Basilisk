#!/usr/bin/env python3
"""Aggregate hyperfine JSON into the git-tracked benchmark status CSV.

Two responsibilities, deliberately DECOUPLED so one can never suppress the other:

  1. WRITE (unconditional, immediate).  On EVERY invocation — `incremental`
     (after each fixture completes) and `final` — the measured numbers are
     written straight to benchmarks/status/<slug>.csv. There is no gate, no
     branch, no "left unchanged" path: the file ALWAYS reflects exactly what
     this build just measured, the instant each score exists. A benchmark run
     that measured a number but did not record it is a lie about the build's
     performance, and the whole point of the suite is to KNOW the moment a
     number slips — so the write happens no matter what the gate later decides.

  2. GATE (CI pass/fail, read-only).  In `final` mode, AFTER the numbers are on
     disk, the run's basilisk times are compared against the COMMITTED baseline
     (the status CSV at BENCH_BASELINE_REF, default HEAD, read from git — never
     the working copy we just overwrote, so a slower run can never launder its
     regression into the baseline). A backwards step beyond BENCH_TOLERANCE_PCT
     on any fixture exits 3 → CI FAILURE. The gate only READS; it never edits
     the file. The committed baseline advances only when a run is committed, so
     it still ratchets toward faster — but the live file never hides a slip.

Usage:  summarize.py <out_dir> <mode> <tool>...     mode ∈ {incremental, final}

All machine/tool metadata + policy arrive via BENCH_* env vars (set by run.sh).
"""

import glob
import json
import os
import subprocess
import sys

STATUS_NOTE = (
    "<tool>_ms = COLD full-file CLI check from scratch (whole process: startup + "
    "stubs + analysis). <tool>_diags = error diagnostics the tool reported on that "
    "fixture in the measured configuration (error severity only; warnings/notes are "
    "not counted) — read every time next to its diags; a tool that reports 0 "
    "analyzed the file but flagged no errors there. A blank _ms cell means the tool "
    "either was not installed on this machine or failed to analyze that fixture "
    "(exit >= 2, e.g. parse abort) and was excluded rather than timed as a crash. "
    "Competitor versions are the LATEST official release pulled at the top of every "
    "run, so their columns always reflect current upstream, never a pinned build. "
    "Only basilisk and mypy have a -warm column (they keep a real cross-run cache): "
    "basilisk-warm = --cache result-cache hit; mypy-warm = incremental .mypy_cache "
    "hit (cold mypy = --no-incremental). pyright/ty/pyrefly keep NO cross-run result "
    "cache (a repeat run = cold), so they are measured cold-only. zuban is also "
    "cold-only but its mypy mode DOES reuse a ./.mypy_cache when present (no flag "
    "disables it), so we wipe ./.mypy_cache before every timed run to keep the "
    "measurement cold. mypy runs with --strict so it performs the strict-mode "
    "analysis the fixtures stress (plain mypy reports 'no issues' on the strictness "
    "fixtures); zuban runs as `zuban mypy --strict` for the same reason (its default "
    "`zuban check` mode skips these strictness rules). This file is ALWAYS rewritten "
    "with the latest measured numbers, even on a regression — the CI gate reads the "
    "committed baseline, never this working copy, so a slip is recorded here AND "
    "fails CI rather than being hidden."
)


def read_rows(out_dir):
    """(stem, {command: mean_ms}) for every per-fixture hyperfine JSON present."""
    rows = []
    for json_file in sorted(glob.glob(os.path.join(out_dir, "*.json"))):
        stem = os.path.splitext(os.path.basename(json_file))[0]
        with open(json_file) as handle:
            data = json.load(handle)
        means = {r["command"]: r["mean"] * 1000.0 for r in data["results"]}
        rows.append((stem, means))
    return rows


def read_coverage():
    """(stem, tool) -> diagnostic count from the preflight coverage.tsv."""
    coverage = {}
    cov_path = os.environ.get("BENCH_COVERAGE", "")
    if cov_path and os.path.exists(cov_path):
        with open(cov_path) as handle:
            for raw in handle:
                parts = raw.rstrip("\n").split("\t")
                if len(parts) == 4:
                    coverage[(parts[0], parts[1])] = parts[3]
    return coverage


def build_csv_lines(rows, all_tools, base_tools, coverage):
    """The full status-schema CSV: metadata header + one row per fixture."""
    lines = [
        f"# machine: {os.environ['BENCH_MACHINE']}",
        f"# cpu: {os.environ['BENCH_CPU']}",
        f"# arch: {os.environ['BENCH_ARCH']}",
        f"# os: {os.environ['BENCH_OS']}",
        f"# cores: {os.environ['BENCH_CORES']}",
        f"# tools: {os.environ['BENCH_TOOLS']}",
        f"# runs: {os.environ['BENCH_RUNS']} minimum; noisy Basilisk CV "
        f"> {float(os.environ['BENCH_MAX_CV']):.0%} is remeasured with at least "
        f"{os.environ['BENCH_STABILITY_RUNS']} runs (hyperfine mean wall-clock, milliseconds)",
        f"# generated: {os.environ['BENCH_GENERATED']}",
        f"# note: {STATUS_NOTE}",
        "fixture,"
        + ",".join(f"{t}_ms" for t in all_tools)
        + ","
        + ",".join(f"{t}_diags" for t in base_tools),
    ]
    for stem, means in rows:
        cells = [stem] + [f"{means[t]:.1f}" if t in means else "" for t in all_tools]
        cells += [coverage.get((stem, t), "") for t in base_tools]
        lines.append(",".join(cells))
    return lines


def write_file(path, lines):
    """Atomic write: tmp + os.replace, so a kill mid-write never tears the file."""
    tmp = f"{path}.tmp"
    with open(tmp, "w") as handle:
        handle.write("\n".join(lines) + "\n")
    os.replace(tmp, path)


def print_console_table(rows, tools):
    width = max(len(s) for s, _ in rows)
    header = f"  {'fixture':<{width}}  " + "  ".join(f"{t:>9}" for t in tools)
    print(header)
    print("  " + "-" * (len(header) - 2))
    for stem, means in rows:
        cells = []
        fastest = min((means[t] for t in tools if t in means), default=None)
        for tool in tools:
            if tool in means:
                mark = (
                    " *"
                    if fastest is not None and abs(means[tool] - fastest) < 1e-9
                    else "  "
                )
                cells.append(f"{means[tool]:7.1f}{mark}")
            else:
                cells.append(f"{'n/a':>9}")
        print(f"  {stem:<{width}}  " + "  ".join(cells))
    print("\n  (* = fastest for that fixture; lower is better)")


def write_summary_md(out_dir, rows, tools):
    lines = [
        "# Benchmark summary\n",
        f"Machine: `{os.environ['BENCH_MACHINE']}`\n",
        "",
        "| fixture | " + " | ".join(tools) + " |",
        "|" + "---|" * (len(tools) + 1),
    ]
    for stem, means in rows:
        lines.append(
            f"| {stem} | "
            + " | ".join(f"{means[t]:.1f} ms" if t in means else "n/a" for t in tools)
            + " |"
        )
    write_file(os.path.join(out_dir, "summary.md"), lines)


def parse_basilisk_ms(text):
    """basilisk_ms per fixture from status-CSV text (the committed baseline)."""
    base, cols = {}, None
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(",")
        if cols is None:
            cols = parts
            continue
        if "basilisk_ms" not in cols:
            break
        idx = cols.index("basilisk_ms")
        val = parts[idx] if idx < len(parts) else ""
        if val:
            base[parts[0]] = float(val)
    return base


def read_committed_baseline(root, rel_path, ref):
    """The COMMITTED status CSV at <ref> (default HEAD), read from git — NOT the
    working copy this run overwrote. Empty dict if the file/ref/repo is absent
    (a machine seeing its first run has no baseline to defend yet)."""
    try:
        result = subprocess.run(
            ["git", "show", f"{ref}:{rel_path}"],
            cwd=root,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError:
        return {}
    if result.returncode != 0:
        return {}
    return parse_basilisk_ms(result.stdout)


def find_regressions(rows, baseline, tolerance_pct):
    """Fixtures slower than the committed baseline beyond ``tolerance_pct``.

    Production runs hard-code that tolerance to zero; the parameter keeps this
    pure comparison helper directly testable.
    """
    regressions = []
    for stem, means in rows:
        if "basilisk" not in means or stem not in baseline:
            continue
        old, new = baseline[stem], means["basilisk"]
        if old > 0 and new > old * (1.0 + tolerance_pct / 100.0):
            regressions.append((stem, old, new, (new / old - 1.0) * 100.0))
    return regressions


def main():
    out_dir = sys.argv[1]
    mode = sys.argv[2]
    tools = sys.argv[3:]
    all_tools = os.environ["BENCH_ALL_TOOLS"].split()
    base_tools = [t for t in all_tools if not t.endswith("-warm")]
    slug = os.environ["BENCH_SLUG"]
    status_dir = os.environ["BENCH_STATUS_DIR"]
    status_path = os.path.join(status_dir, f"{slug}.csv")

    rows = read_rows(out_dir)
    if not rows:
        if mode == "final":
            print("  (no JSON results)")
        return 0

    coverage = read_coverage()
    csv_lines = build_csv_lines(rows, all_tools, base_tools, coverage)

    # (1) WRITE — unconditional, immediate. The live file always tells the truth
    # about this build, on every invocation, regardless of what the gate decides.
    write_file(status_path, csv_lines)

    if mode != "final":
        return 0

    # (2) GATE — read-only, AFTER the write. Compare against the COMMITTED
    # baseline (from git), never the working copy we just overwrote.
    print_console_table(rows, tools)
    write_summary_md(out_dir, rows, tools)

    root = os.environ["BENCH_ROOT"]
    ref = os.environ.get("BENCH_BASELINE_REF", "HEAD")
    tolerance_pct = float(os.environ.get("BENCH_TOLERANCE_PCT", "0"))
    rel_path = os.path.relpath(status_path, root)
    baseline = read_committed_baseline(root, rel_path, ref)
    regressions = find_regressions(rows, baseline, tolerance_pct)

    print(f"\n  Status CSV (written, git-tracked): {status_path}")
    print(f"  Summary:                  {os.path.join(out_dir, 'summary.md')}")
    if regressions:
        print(
            f"\n  REGRESSION GATE — basilisk slower than the COMMITTED baseline "
            f"({ref}) by >{tolerance_pct:.0f}%"
        )
        print(
            "    The slower numbers are ALREADY written above — this fails CI so the slip is visible, not hidden."
        )
        print(f"    {'fixture':<34} {'baseline':>11} {'now':>11} {'change':>9}")
        for stem, old, new, delta in regressions:
            print(f"    {stem:<34} {old:>8.1f} ms {new:>8.1f} ms {delta:>+7.1f}%")
        print("    Optimize the regression, then commit the file to move the baseline.")
        return 3
    if baseline:
        print(
            f"\n  No regression vs committed baseline ({ref}) — within {tolerance_pct:.0f}% on every fixture."
        )
    else:
        print(
            f"\n  No committed baseline at {ref} for this machine yet — this run establishes it once committed."
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
