#!/usr/bin/env bash
# Benchmark: Basilisk vs Pyright vs mypy vs ty vs Pyrefly.
#
# Uses hyperfine for accurate wall-clock timing. Each fixture is a large
# single-rule stress file (2k–3.5k lines) so the numbers reflect steady-state
# checking throughput rather than process startup.
#
# Run from the repo root:  make bench   (or:  bash benchmarks/run.sh)
#
# OUTPUT (auto-generated every run):
#   benchmarks/status/<machine>.csv   — git-tracked per-machine results table,
#                                        the same way conformance_status.csv is
#                                        tracked. The website reads this file, so
#                                        the published numbers are never hand-typed.
#   benchmarks/results/*.json         — raw hyperfine output (gitignored)
#   benchmarks/results/summary.md     — human-readable summary (gitignored)
#
# Competitor tools are OPTIONAL — any that are not installed are skipped (their
# column is left blank). Only `basilisk` and `hyperfine` are required. Fixtures
# intentionally contain errors, so every command runs with --ignore-failure.
#
# REGRESSION GATE:
#   `make bench` FAILS (non-zero exit) if basilisk got slower than the recorded
#   baseline (the existing benchmarks/status/<machine>.csv) on any fixture by
#   more than BENCH_REGRESS_PCT (default 25%). On a regression the baseline CSV
#   is left UNCHANGED so the gate keeps comparing against known-good numbers.
#   To accept new numbers / establish a fresh baseline (e.g. after changing the
#   fixture set or intentionally trading speed for correctness):
#       BENCH_NO_GATE=1 make bench
#   Knobs:  BENCH_REGRESS_PCT=<pct>   BENCH_NO_GATE=1   RUNS=<n>   WARMUP=<n>

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BSK="$ROOT/target/release/basilisk"
FX="$ROOT/benchmarks/fixtures"
OUT="$ROOT/benchmarks/results"
STATUS_DIR="$ROOT/benchmarks/status"
RUNS="${RUNS:-10}"
WARMUP="${WARMUP:-2}"
# Regression gate: on by default; BENCH_NO_GATE=1 disables it and re-baselines.
BENCH_GATE="1"; [[ -n "${BENCH_NO_GATE:-}" ]] && BENCH_GATE="0"
BENCH_REGRESS_PCT="${BENCH_REGRESS_PCT:-25}"
mkdir -p "$OUT" "$STATUS_DIR"

# Canonical tool column order for the status CSV / website (stable schema).
ALL_TOOLS="basilisk pyright mypy ty pyrefly"

# ─── Preconditions ────────────────────────────────────────────────────────────
if ! command -v hyperfine >/dev/null 2>&1; then
  echo "ERROR: hyperfine is not installed (brew install hyperfine / cargo install hyperfine)." >&2
  exit 1
fi
if [[ ! -x "$BSK" ]]; then
  echo "ERROR: basilisk release binary not found at $BSK" >&2
  echo "       Build it first:  cargo build --release --bin basilisk" >&2
  exit 1
fi

# ─── Tool discovery (basilisk required, competitors optional) ─────────────────
# Each entry: "name|command-template"  where {} is replaced by the fixture path.
declare -a TOOL_NAMES=() TOOL_CMDS=()
add_tool() { TOOL_NAMES+=("$1"); TOOL_CMDS+=("$2"); }

add_tool "basilisk" "$BSK check {}"
command -v pyright  >/dev/null 2>&1 && add_tool "pyright"  "pyright {}"
command -v mypy     >/dev/null 2>&1 && add_tool "mypy"     "mypy --ignore-missing-imports --no-error-summary {}"
command -v ty       >/dev/null 2>&1 && add_tool "ty"       "ty check {}"
command -v pyrefly  >/dev/null 2>&1 && add_tool "pyrefly"  "pyrefly check {}"

version_of() {
  local v=""
  case "$1" in
    basilisk) v="$("$BSK" --version 2>&1 | head -1)" ;;
    *)        v="$("$1" --version 2>&1 | head -1)" ;;
  esac
  printf '%s' "${v:-n/a}"
}

# ─── Machine identity & conditions (no hostname — CPU/arch/OS only) ───────────
slugify() { printf '%s' "$1" | tr 'A-Z' 'a-z' | tr -cs 'a-z0-9' '-' | sed 's/^-*//; s/-*$//'; }
detect_cpu() {
  if sysctl -n machdep.cpu.brand_string >/dev/null 2>&1; then
    sysctl -n machdep.cpu.brand_string
  elif [[ -r /proc/cpuinfo ]]; then
    grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ *//'
  else
    echo "unknown CPU"
  fi
}
detect_cores() { sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo "?"; }

BENCH_CPU="$(detect_cpu)"
BENCH_ARCH="$(uname -m)"
BENCH_OS="$(uname -s) $(uname -r)"
BENCH_CORES="$(detect_cores)"
BENCH_GENERATED="$(date '+%Y-%m-%dT%H:%M:%S%z')"
if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
  BENCH_MACHINE="GitHub Actions ${RUNNER_NAME:-runner} (${RUNNER_OS:-?}/${RUNNER_ARCH:-?})"
  BENCH_SLUG="gha-$(slugify "${RUNNER_OS:-unknown}-${RUNNER_ARCH:-unknown}")"
else
  BENCH_MACHINE="$BENCH_CPU"
  BENCH_SLUG="$(slugify "$(uname -s)-${BENCH_ARCH}-${BENCH_CPU}")"
fi

# Tool-version string for the CSV header.
BENCH_TOOLS=""
for name in "${TOOL_NAMES[@]}"; do
  BENCH_TOOLS+="${BENCH_TOOLS:+, }${name}=$(version_of "$name")"
done

echo "======================================================="
echo "  Basilisk Benchmark — ${BENCH_GENERATED}"
echo "======================================================="
echo "  machine : $BENCH_MACHINE"
echo "  cpu     : $BENCH_CPU ($BENCH_CORES cores, $BENCH_ARCH)"
echo "  os      : $BENCH_OS"
echo "  slug    : $BENCH_SLUG  ->  benchmarks/status/${BENCH_SLUG}.csv"
echo "  runs    : $RUNS (warmup $WARMUP)"
echo "  tools   :"
for name in "${TOOL_NAMES[@]}"; do
  printf "    %-9s %s\n" "$name" "$(version_of "$name")"
done
echo ""

# ─── Per-file benchmarks ──────────────────────────────────────────────────────
# (portable fixture collection — macOS ships bash 3.2, which lacks `mapfile`)
FIXTURES=()
while IFS= read -r _f; do
  [[ -n "$_f" ]] && FIXTURES+=("$_f")
done < <(cd "$FX" && ls -1 ./*.py 2>/dev/null | sed 's|^\./||' | sort)
if [[ ${#FIXTURES[@]} -eq 0 ]]; then
  echo "ERROR: no fixtures found in $FX" >&2
  exit 1
fi

echo "─── Per-file timing ($RUNS runs each) ──────────────────────────────────"
echo ""

for FILE in "${FIXTURES[@]}"; do
  FPATH="$FX/$FILE"
  STEM="${FILE%.py}"
  echo "┌─ $STEM ($(wc -l < "$FPATH" | tr -d ' ') lines)"

  HF=(hyperfine --ignore-failure --warmup "$WARMUP" --runs "$RUNS"
      --export-json "$OUT/${STEM}.json")
  for i in "${!TOOL_NAMES[@]}"; do
    CMD="${TOOL_CMDS[$i]//\{\}/$FPATH}"
    HF+=(--command-name "${TOOL_NAMES[$i]}" "$CMD")
  done

  "${HF[@]}" 2>&1 | grep -E "Time|Summary|ran|faster|slower" | sed 's/^/│  /' || true
  echo "└──"
  echo ""
done

# ─── Write outputs: summary.md (ephemeral) + status CSV (git-tracked) ─────────
echo "─── Summary: mean wall-clock per fixture (ms) ──────────────────────────"
echo ""
BENCH_SLUG="$BENCH_SLUG" BENCH_MACHINE="$BENCH_MACHINE" BENCH_CPU="$BENCH_CPU" \
BENCH_ARCH="$BENCH_ARCH" BENCH_OS="$BENCH_OS" BENCH_CORES="$BENCH_CORES" \
BENCH_GENERATED="$BENCH_GENERATED" BENCH_TOOLS="$BENCH_TOOLS" BENCH_RUNS="$RUNS" \
BENCH_STATUS_DIR="$STATUS_DIR" BENCH_ALL_TOOLS="$ALL_TOOLS" \
BENCH_GATE="$BENCH_GATE" BENCH_REGRESS_PCT="$BENCH_REGRESS_PCT" \
python3 - "$OUT" "${TOOL_NAMES[@]}" <<'PY'
import json, os, sys, glob

out_dir = sys.argv[1]
tools = sys.argv[2:]
all_tools = os.environ["BENCH_ALL_TOOLS"].split()

rows = []
for jf in sorted(glob.glob(os.path.join(out_dir, "*.json"))):
    stem = os.path.splitext(os.path.basename(jf))[0]
    with open(jf) as fh:
        data = json.load(fh)
    means = {r["command"]: r["mean"] * 1000.0 for r in data["results"]}
    rows.append((stem, means))

if not rows:
    print("  (no JSON results)")
    raise SystemExit

# Console table -------------------------------------------------------------
w = max(len(s) for s, _ in rows)
header = f"  {'fixture':<{w}}  " + "  ".join(f"{t:>9}" for t in tools)
print(header)
print("  " + "-" * (len(header) - 2))
for stem, means in rows:
    cells = []
    fastest = min((means[t] for t in tools if t in means), default=None)
    for t in tools:
        if t in means:
            mark = " *" if fastest is not None and abs(means[t] - fastest) < 1e-9 else "  "
            cells.append(f"{means[t]:7.1f}{mark}")
        else:
            cells.append(f"{'n/a':>9}")
    print(f"  {stem:<{w}}  " + "  ".join(cells))
print("\n  (* = fastest for that fixture; lower is better)")

# Human-readable summary.md (gitignored) ------------------------------------
md = ["# Benchmark summary\n", f"Machine: `{os.environ['BENCH_MACHINE']}`\n", "",
      "| fixture | " + " | ".join(tools) + " |",
      "|" + "---|" * (len(tools) + 1)]
for stem, means in rows:
    md.append(f"| {stem} | " + " | ".join(f"{means[t]:.1f} ms" if t in means else "n/a" for t in tools) + " |")
with open(os.path.join(out_dir, "summary.md"), "w") as fh:
    fh.write("\n".join(md) + "\n")

# Git-tracked per-machine status CSV + regression gate ----------------------
# Stable schema: a metadata header (# lines) the website parses for conditions,
# then one row per fixture with a fixed `<tool>_ms` column for every tool.
status_path = os.path.join(os.environ["BENCH_STATUS_DIR"], os.environ["BENCH_SLUG"] + ".csv")
gate_on = os.environ.get("BENCH_GATE", "1") == "1"
pct = float(os.environ.get("BENCH_REGRESS_PCT", "25"))

def read_baseline_basilisk(path):
    """basilisk_ms per fixture from an existing status CSV (the last/committed run)."""
    base, cols = {}, None
    if not os.path.exists(path):
        return base
    with open(path) as fh:
        for raw in fh:
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

baseline = read_baseline_basilisk(status_path)

# Regression = basilisk slower than baseline by more than `pct`% on a fixture.
regressions = []
for stem, means in rows:
    if "basilisk" not in means or stem not in baseline:
        continue
    old, new = baseline[stem], means["basilisk"]
    if old > 0 and new > old * (1.0 + pct / 100.0):
        regressions.append((stem, old, new, (new / old - 1.0) * 100.0))

csv_lines = [
    f"# machine: {os.environ['BENCH_MACHINE']}",
    f"# cpu: {os.environ['BENCH_CPU']}",
    f"# arch: {os.environ['BENCH_ARCH']}",
    f"# os: {os.environ['BENCH_OS']}",
    f"# cores: {os.environ['BENCH_CORES']}",
    f"# tools: {os.environ['BENCH_TOOLS']}",
    f"# runs: {os.environ['BENCH_RUNS']} (hyperfine mean wall-clock, milliseconds)",
    f"# generated: {os.environ['BENCH_GENERATED']}",
    f"# note: cold full-file CLI runs; does not reflect warm incremental (LSP) speed.",
    "fixture," + ",".join(f"{t}_ms" for t in all_tools),
]
for stem, means in rows:
    cells = [stem] + [f"{means[t]:.1f}" if t in means else "" for t in all_tools]
    csv_lines.append(",".join(cells))

blocked = bool(regressions) and gate_on
if blocked:
    print(f"\n  REGRESSION GATE — basilisk slower than baseline by >{pct:.0f}%")
    print(f"    baseline: {status_path}")
    print(f"    {'fixture':<34} {'baseline':>11} {'now':>11} {'change':>9}")
    for stem, old, new, delta in regressions:
        print(f"    {stem:<34} {old:>8.1f} ms {new:>8.1f} ms {delta:>+7.1f}%")
    print("    Baseline left UNCHANGED. Investigate, or re-baseline with:")
    print("      BENCH_NO_GATE=1 make bench")
else:
    with open(status_path, "w") as fh:
        fh.write("\n".join(csv_lines) + "\n")
    suffix = "  (gate off; baseline re-set)" if (regressions and not gate_on) else ""
    print(f"\n  Status CSV (git-tracked): {status_path}{suffix}")
print(f"  Summary:                  {os.path.join(out_dir, 'summary.md')}")

if blocked:
    raise SystemExit(3)
PY
GATE_STATUS=$?

# ─── Diagnostic coverage (best-effort error counts) ───────────────────────────
echo ""
echo "─── Diagnostic coverage (errors reported per tool) ─────────────────────"
echo ""
printf "  %-38s" "fixture"
for name in "${TOOL_NAMES[@]}"; do printf " %9s" "$name"; done
echo ""
for FILE in "${FIXTURES[@]}"; do
  FPATH="$FX/$FILE"
  printf "  %-38s" "${FILE%.py}"
  for i in "${!TOOL_NAMES[@]}"; do
    name="${TOOL_NAMES[$i]}"
    CMD="${TOOL_CMDS[$i]//\{\}/$FPATH}"
    case "$name" in
      basilisk) n=$($CMD 2>/dev/null | grep -cE "error\[BSK" || true) ;;
      pyright)  n=$($CMD 2>/dev/null | grep -cE " - error:" || true) ;;
      mypy)     n=$($CMD 2>/dev/null | grep -cE ": error:" || true) ;;
      ty)       n=$($CMD 2>/dev/null | grep -cE "error\[|error:" || true) ;;
      pyrefly)  n=$($CMD 2>/dev/null | grep -cE "error\[|ERROR " || true) ;;
      *)        n=0 ;;
    esac
    printf " %9s" "$n"
  done
  echo ""
done

echo ""
if [[ "${GATE_STATUS:-0}" -ne 0 ]]; then
  echo "RESULT: FAIL — performance regression vs baseline (see gate report above)."
  echo "        Fix the slowdown, or re-baseline with: BENCH_NO_GATE=1 make bench"
else
  echo "RESULT: PASS — no performance regression vs baseline."
  echo "        Commit benchmarks/status/*.csv to track the trend."
fi
exit "${GATE_STATUS:-0}"
