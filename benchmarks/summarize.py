#!/usr/bin/env python3
"""Aggregate hyperfine JSON into the git-tracked benchmark status CSV.

WRITE (unconditional, immediate).  On EVERY invocation — `incremental` (after
each fixture completes) and `final` — the measured numbers are written straight
to benchmarks/status/<slug>.csv. There is no branch and no "left unchanged"
path: the file ALWAYS reflects exactly what this build just measured, the
instant each score exists. A benchmark run that measured a number but did not
record it is a lie about the build's performance.

THIS SCRIPT DOES NOT GATE ANYTHING, and nothing in CI fails on a benchmark
number.  The benchmark is an INDICATIVE, developer-run measurement: `make bench`
executes on whatever workstation a contributor happens to use, against whatever
else that machine is doing at the time. Background load moves every tool in the
table together and can shift absolute times by tens of percent between two runs
of identical code, which is far larger than the changes worth acting on. A
pass/fail gate built on that signal fails honest work and passes real
regressions depending on what else was running, so there is no gate to tune,
disable, or widen — the numbers are reported, and a human reads them.

Compare tools WITHIN one run (they are measured back to back on the same
machine, so machine speed cancels); do not compare a number from one run against
a number recorded on a different machine or at a different time. When a real
performance question needs an answer, measure both revisions on one quiet
machine in one sitting.

CARRY-FORWARD.  A run may deliberately measure only some tools — `make
bench-basilisk` re-times basilisk alone, because five 0.5s-per-invocation
competitors add minutes to every iteration and prove nothing about a change to
this tree. With BENCH_CARRY_FROM set, the cells of the tools this run did not
measure are copied verbatim out of the pre-run snapshot of the status CSV
instead of being emptied: a blank cell means "not installed / failed preflight"
and must
keep meaning exactly that. Nothing measured is ever carried over — a carried
cell is by definition one this run produced no number for — so the write-always
rule above is untouched. The header records which tools the run measured and
which it carried, so the file never implies a competitor was re-timed.

Usage:  summarize.py <out_dir> <mode> <tool>...     mode ∈ {incremental, final}

All machine/tool metadata + policy arrive via BENCH_* env vars (set by run.sh).
"""

import glob
import json
import os
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
    "with the latest measured numbers. These are INDICATIVE developer-machine "
    "measurements, not a gate: nothing in CI passes or fails on them. Background load "
    "moves every tool in the table together and can shift absolute times by tens of "
    "percent between two runs of identical code, so compare tools WITHIN one run "
    "(measured back to back on the same machine) and never against numbers recorded "
    "on another machine or at another time."
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


def build_carry(all_tools, measured):
    """Carry-forward source for the tools this run deliberately did not measure.

    Reads BENCH_CARRY_FROM — run.sh's snapshot of the status CSV as it stood
    BEFORE this run's first write, never the live file. Carrying from the live
    file would mean each fixture's incremental write carried from the write
    before it, so only the first fixture would still find the competitor cells.

    Empty (falsy) when that variable is unset, so a full sweep keeps blanking
    the column of a competitor that is merely absent.
    """
    source = os.environ.get("BENCH_CARRY_FROM", "")
    if not source or not os.path.exists(source):
        return {}
    with open(source) as handle:
        text = handle.read()
    header, cells = parse_status_csv(text)
    return {
        "tools": {t for t in all_tools if t not in measured},
        "cells": cells,
        "tools_header": header.get("tools", ""),
        "generated": header.get("generated", ""),
    }


def parse_status_csv(text):
    """A status CSV's `# key: value` header and its {(fixture, column): cell}."""
    header, cells, columns = {}, {}, None
    for raw in text.splitlines():
        line = raw.strip()
        if line.startswith("#"):
            key, sep, value = line.lstrip("# ").partition(":")
            if sep:
                header[key.strip()] = value.strip()
        elif line and columns is None:
            columns = line.split(",")
        elif line:
            parts = line.split(",")
            for column, cell in zip(columns[1:], parts[1:]):
                cells[(parts[0], column)] = cell
    return header, cells


def carry_values(rows, all_tools, carry):
    """This run's timings with the carried-forward `_ms` cells folded in, so the
    CSV and summary render one complete table."""
    if not carry:
        return rows
    filled = []
    for stem, means in rows:
        merged = dict(means)
        for tool in carry["tools"]:
            cell = carry["cells"].get((stem, f"{tool}_ms"), "")
            if cell and tool in all_tools:
                merged[tool] = float(cell)
        filled.append((stem, merged))
    return filled


def columns_with_values(rows, all_tools):
    """Tools that hold a value on at least one fixture, in canonical order."""
    return [t for t in all_tools if any(t in means for _, means in rows)]


def build_csv_lines(rows, all_tools, base_tools, coverage, carry):
    """The full status-schema CSV: metadata header + one row per fixture."""
    lines = [
        f"# machine: {os.environ['BENCH_MACHINE']}",
        f"# cpu: {os.environ['BENCH_CPU']}",
        f"# arch: {os.environ['BENCH_ARCH']}",
        f"# os: {os.environ['BENCH_OS']}",
        f"# cores: {os.environ['BENCH_CORES']}",
        f"# tools: {build_tools_header(base_tools, carry)}",
        f"# runs: {os.environ['BENCH_RUNS']} minimum; noisy Basilisk CV "
        f"> {float(os.environ['BENCH_MAX_CV']):.0%} is remeasured with at least "
        f"{os.environ['BENCH_STABILITY_RUNS']} runs (hyperfine mean wall-clock, milliseconds)",
        f"# generated: {os.environ['BENCH_GENERATED']}",
    ]
    lines += provenance_header(all_tools, carry)
    lines += [
        f"# note: {STATUS_NOTE}",
        "fixture,"
        + ",".join(f"{t}_ms" for t in all_tools)
        + ","
        + ",".join(f"{t}_diags" for t in base_tools),
    ]
    for stem, means in rows:
        cells = [stem] + [f"{means[t]:.1f}" if t in means else "" for t in all_tools]
        cells += [diag_cell(stem, t, coverage, carry) for t in base_tools]
        lines.append(",".join(cells))
    return lines


def provenance_header(all_tools, carry):
    """The `# measured:` line, present only when this run measured a subset.

    Without it a partial run's CSV would read as a full sweep at the fresh
    `# generated` stamp. Names both halves so a reader can always tell which
    columns this build produced and which predate it.
    """
    if not carry:
        return []
    carried = [t for t in all_tools if t in carry["tools"]]
    measured = [t for t in all_tools if t not in carry["tools"]]
    return [
        f"# measured: {', '.join(measured)} — timed by this run. "
        f"{', '.join(carried)} were NOT re-timed; their _ms and _diags cells and "
        f"version strings are carried forward verbatim from the previous run "
        f"({carry['generated'] or 'unknown date'}) on this machine."
    ]


def diag_cell(stem, tool, coverage, carry):
    """A tool's diagnostic count: this run's preflight, else the carried cell.

    Membership in ``coverage`` — not truthiness — decides, so a measured tool
    that crashed preflight keeps its deliberate blank instead of resurrecting a
    stale count from the file.
    """
    if (stem, tool) in coverage:
        return coverage[(stem, tool)]
    return carry["cells"].get((stem, f"{tool}_diags"), "") if carry else ""


def build_tools_header(base_tools, carry):
    """`# tools:` — this run's versions, keeping the previous file's entry for
    any tool it carried forward (which is never one this run measured)."""
    if not carry:
        return os.environ["BENCH_TOOLS"]
    entries = tool_entries(carry["tools_header"])
    entries.update(tool_entries(os.environ["BENCH_TOOLS"]))
    return ", ".join(f"{t}={entries[t]}" for t in base_tools if t in entries)


def tool_entries(text):
    """`name=version` pairs from a `# tools:` header line.

    Split on ", " and re-attach any chunk that is not a fresh `name=` to the
    entry before it, so a version string carrying a comma survives the round
    trip intact.
    """
    entries, last = {}, None
    for chunk in text.split(", "):
        name, sep, version = chunk.partition("=")
        if sep and " " not in name:
            entries[name], last = version, name
        elif last is not None:
            entries[last] += f", {chunk}"
    return entries


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
    # Read the carry-forward source BEFORE the write below overwrites it.
    carry = build_carry(all_tools, tools)
    published = carry_values(rows, all_tools, carry)
    csv_lines = build_csv_lines(published, all_tools, base_tools, coverage, carry)

    # WRITE — unconditional, immediate. The live file always tells the truth
    # about this build, on every invocation.
    write_file(status_path, csv_lines)

    if mode != "final":
        return 0

    # Report only. There is no gate: these are indicative developer-machine
    # numbers and nothing passes or fails on them.
    print_console_table(rows, tools)
    write_summary_md(out_dir, published, columns_with_values(published, all_tools))

    print(f"\n  Status CSV (written, git-tracked): {status_path}")
    print(f"  Summary:                  {os.path.join(out_dir, 'summary.md')}")
    print(
        "\n  Indicative only — measured on this machine, under whatever else it was\n"
        "  running. Compare tools within this run; do not compare against numbers\n"
        "  recorded on another machine or at another time."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
