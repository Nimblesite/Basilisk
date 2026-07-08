#!/usr/bin/env bash
# Benchmark: Basilisk vs Pyright vs mypy vs ty vs Pyrefly vs Zuban.
#
# Uses hyperfine for accurate wall-clock timing. Each fixture is a large
# single-construct stress file (2k–4k lines) that exercises one typing-spec
# construct. Timings are whole-process cold checks — startup + stubs + analysis
# — i.e. the real latency of checking one file from scratch. For heavier tools
# that latency is dominated by their fixed startup/stub cost, NOT by the
# fixture's construct; the published tables therefore compare per-tool
# distributions (median/range), never construct-vs-construct across tools.
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
# intentionally contain errors, so tools exit 1 and every hyperfine command runs
# with --ignore-failure. That flag would also happily time a CRASH, so a
# preflight pass first runs every tool on every fixture un-timed: exit >= 2
# (parse abort / internal error / bad usage) excludes that tool from that
# fixture's timing and leaves its CSV cell blank — a tool that never analyzed
# a file must not be published as a (fast) time for it. The same pass records
# how many diagnostics each tool reports per fixture (the <tool>_diags CSV
# columns), so a do-nothing run is visible in the published data, not hidden.
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
# Persistent cache dirs for the warm columns of the tools that HAVE a result
# cache (basilisk's --cache, mypy's incremental). Entries are keyed by the
# target path, so one dir across fixtures never collides; the hyperfine warmup
# runs populate them so the measured runs are cache hits.
WARMCACHE="$OUT/.warmcache"
MYPYCACHE="$OUT/.mypycache"
# Regression gate: on by default; BENCH_NO_GATE=1 disables it and re-baselines.
BENCH_GATE="1"; [[ -n "${BENCH_NO_GATE:-}" ]] && BENCH_GATE="0"
BENCH_REGRESS_PCT="${BENCH_REGRESS_PCT:-25}"
mkdir -p "$OUT" "$STATUS_DIR"

# Canonical tool column order for the status CSV / website (stable schema).
# A tool gets a -warm column ONLY if it keeps a real cross-run cache whose
# repeat-run speed actually differs from a cold check. Otherwise the warm column
# is just a duplicate of cold — pure noise — so we don't measure it. Only two
# tools qualify:
#   * basilisk-warm  — opt-in result-cache hit (basilisk check --cache)
#   * mypy-warm      — incremental .mypy_cache hit; cold mypy is --no-incremental
# pyright/ty/pyrefly keep NO cross-run result cache (verified: zero cache
# artifacts), so a repeat run lands ~= cold — they are measured COLD-only.
# zuban is ALSO measured cold-only, but it is the one cold tool that DOES emit a
# cross-run cache: `zuban mypy` mirrors mypy and reads/writes a `.mypy_cache` in
# the working dir whenever one is present, and it exposes no --cache-dir /
# --no-incremental flag to opt out (verified: both rejected as unknown args; it
# also ignores $MYPY_CACHE_DIR). Left alone, hyperfine's warmup runs would prime
# that cache and turn every measured run into a warm incremental hit — the exact
# apples-to-oranges trap we avoid for mypy with --no-incremental. So we bust the
# cache before EVERY timed run (see the `--prepare` on the per-fixture hyperfine
# call below), forcing the COLD label to hold by construction — zuban gets no
# published warm number because none is measured, never because we assume its
# cache is worthless.
# mypy is run with --strict: without it, mypy reports "no issues" on the
# strictness fixtures (e.g. missing-parameter annotations) and "checks" nothing,
# making its timing an apples-to-oranges lie. --strict makes mypy perform the
# strict-mode analysis these fixtures exist to stress, matching basilisk's
# strict-by-default workload.
# zuban is run as `zuban mypy --strict` for exactly the same reason: its default
# `zuban check` mode skips these strictness rules (it flags 0 errors on the
# missing-annotation fixtures), so only the Mypy-compatible strict mode performs
# the analysis being measured. `zuban mypy` is the strict-mode workload, just as
# `mypy --strict` is — they are the apples-to-apples pair.
ALL_TOOLS="basilisk basilisk-warm pyright mypy mypy-warm ty pyrefly zuban"

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

# Warm caches start empty; the hyperfine warmup populates them so the measured
# warm runs are hits.
rm -rf "$WARMCACHE" "$MYPYCACHE"; mkdir -p "$WARMCACHE" "$MYPYCACHE"
add_tool "basilisk"      "$BSK check {}"
add_tool "basilisk-warm" "$BSK check {} --cache --cache-dir $WARMCACHE"
if command -v pyright >/dev/null 2>&1; then
  # No cross-run result cache → cold-only (a repeat run would just equal cold).
  add_tool "pyright" "pyright {}"
fi
if command -v mypy >/dev/null 2>&1; then
  # --strict so mypy does the strict-mode analysis the fixtures stress; plain
  # mypy reports "no issues" on missing-annotation fixtures and times nothing.
  # cold = --no-incremental (full check); warm = incremental .mypy_cache hit.
  # Without --no-incremental the hyperfine warmup turned every cold measurement
  # into a do-nothing cache hit, which is why mypy used to look flat/fast.
  # No --ignore-missing-imports: the unresolved_imports fixture exists to
  # stress failed-import handling, and that flag would silence exactly that
  # analysis for mypy while every other tool performs it — a hidden workload
  # difference the _diags columns would then misreport as tool behavior.
  add_tool "mypy"      "mypy --strict --no-incremental --no-error-summary {}"
  add_tool "mypy-warm" "mypy --strict --cache-dir $MYPYCACHE --no-error-summary {}"
fi
if command -v ty >/dev/null 2>&1; then
  # No cross-run result cache → cold-only.
  add_tool "ty" "ty check {}"
fi
if command -v pyrefly >/dev/null 2>&1; then
  # No cross-run result cache → cold-only.
  add_tool "pyrefly" "pyrefly check {}"
fi
if command -v zuban >/dev/null 2>&1; then
  # `zuban mypy --strict` (alias of `zmypy --strict`) so it performs the
  # strict-mode analysis the fixtures stress — zuban's default `zuban check`
  # mode skips these strictness rules and reports "no issues" on the
  # missing-annotation fixtures, which would time a do-nothing run, exactly as
  # plain (non-strict) mypy would.
  # Unlike pyright/ty/pyrefly, zuban's mypy mode REUSES a `.mypy_cache` in the cwd
  # whenever one is present (it has no flag to disable it) — and the cold-mypy
  # column scribbles exactly such a cache earlier in the same hyperfine run, so
  # without intervention zuban would read it back and be measured WARM. We force
  # it COLD by wiping that cache before every timed run (ZUBAN_PRESENT gates the
  # `--prepare` on the hyperfine call), keeping the COLD column honest by
  # construction.
  # No --ignore-missing-imports here either — same reason as mypy above.
  ZUBAN_PRESENT=1
  add_tool "zuban" "zuban mypy --strict --no-error-summary {}"
fi

# Clean up the ./.mypy_cache that the cold-mypy column writes (mypy scribbles one
# even under --no-incremental) and that zuban's mypy mode would reuse — so a
# benchmark run never leaves cache litter in the repo. Gated on those two tools
# being measured so we don't delete an unrelated cache when neither ran. The trap
# fires on every exit path, including the regression-gate failure (exit 3).
case " ${TOOL_NAMES[*]} " in
  *" mypy "*|*" zuban "*) trap 'rm -rf .mypy_cache' EXIT ;;
esac

version_of() {
  local base="${1%-warm}" v=""
  case "$base" in
    basilisk)
      v="$("$BSK" --version 2>&1 | head -1)"
      # Dev builds carry the release-time version sentinel (0.0.0-PLACEHOLDER),
      # which says nothing about WHICH basilisk was measured. Pin those to the
      # source commit so the published benchmark records the exact build — a
      # plain placeholder is useless next to competitors' real version numbers.
      if printf '%s' "$v" | grep -qi placeholder; then
        local sha dirty=""
        sha="$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || printf 'unknown')"
        git -C "$ROOT" diff --quiet HEAD 2>/dev/null || dirty="-dirty"
        v="basilisk 0.0.0-dev+g${sha}${dirty}"
      fi
      ;;
    *) v="$("$base" --version 2>&1 | head -1)" ;;
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

# Tool-version string for the CSV header (one entry per base tool; the `-warm`
# variants share the same binary/version, so they are omitted to avoid clutter).
BENCH_TOOLS=""
for name in "${TOOL_NAMES[@]}"; do
  case "$name" in *-warm) continue ;; esac
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

# ─── Preflight: diagnostic coverage + crash screening ─────────────────────────
# One un-timed run of every base tool on every fixture BEFORE anything is timed.
# Two jobs:
#   1. Validity — exit >= 2 means the tool did NOT analyze the file (parse
#      abort, internal error, bad usage; exit 0/1 = analyzed, clean/findings).
#      An invalid tool is excluded from that fixture's hyperfine run, together
#      with its -warm sibling (same binary): --ignore-failure would otherwise
#      record the crash as a (fast) timing. Discovered the hard way: a `# type:`
#      comment made mypy abort at line 1 and its "152 ms" went to the website.
#   2. Coverage — the diagnostic count per tool per fixture goes into the CSV
#      (<tool>_diags), so a tool that runs but finds nothing on a fixture is
#      visible in the published data next to its time.
# (The pyright pip wrapper exits 0 even with findings, so 0 vs 1 carries no
# meaning across tools — only >= 2 does.)
COVERAGE="$OUT/coverage.tsv"
: > "$COVERAGE"
INVALID_COMBOS=" "
echo "─── Preflight: diagnostics per tool + crash screening ──────────────────"
echo ""
printf "  %-38s" "fixture"
for name in "${TOOL_NAMES[@]}"; do
  case "$name" in *-warm) continue ;; esac
  printf " %9s" "$name"
done
echo ""
for FILE in "${FIXTURES[@]}"; do
  FPATH="$FX/$FILE"
  STEM="${FILE%.py}"
  printf "  %-38s" "$STEM"
  for i in "${!TOOL_NAMES[@]}"; do
    name="${TOOL_NAMES[$i]}"
    case "$name" in *-warm) continue ;; esac
    CMD="${TOOL_CMDS[$i]//\{\}/$FPATH}"
    output="$($CMD 2>/dev/null)"; code=$?
    case "$name" in
      # Conformance rules use their rule name as the code (error[typeddicts_inheritance]);
      # house rules use BSK-XXXX (error[BSK-E0001]) — match any error[...] diagnostic.
      basilisk) n=$(printf '%s\n' "$output" | grep -cE "^error\[" || true) ;;
      pyright)  n=$(printf '%s\n' "$output" | grep -cE " - error:" || true) ;;
      mypy)     n=$(printf '%s\n' "$output" | grep -cE ": error:" || true) ;;
      ty)       n=$(printf '%s\n' "$output" | grep -cE "error\[|error:" || true) ;;
      pyrefly)  n=$(printf '%s\n' "$output" | grep -cE "error\[|ERROR " || true) ;;
      zuban)    n=$(printf '%s\n' "$output" | grep -cE ": error:" || true) ;;
      *)        n=0 ;;
    esac
    if [[ "$code" -ge 2 ]]; then
      INVALID_COMBOS+="${STEM}/${name} "
      printf " %9s" "CRASH"
      printf '%s\t%s\t%s\t\n' "$STEM" "$name" "$code" >> "$COVERAGE"
    else
      printf " %9s" "$n"
      printf '%s\t%s\t%s\t%s\n' "$STEM" "$name" "$code" "$n" >> "$COVERAGE"
    fi
  done
  echo ""
done
for combo in $INVALID_COMBOS; do
  echo "  !! ${combo#*/} exited >= 2 on ${combo%/*} — did not analyze the file; timing cell left blank"
done
echo ""

echo "─── Per-file timing ($RUNS runs each) ──────────────────────────────────"
echo ""

for FILE in "${FIXTURES[@]}"; do
  FPATH="$FX/$FILE"
  STEM="${FILE%.py}"
  echo "┌─ $STEM ($(wc -l < "$FPATH" | tr -d ' ') lines)"

  HF=(hyperfine --ignore-failure --warmup "$WARMUP" --runs "$RUNS"
      --export-json "$OUT/${STEM}.json")
  # When zuban is measured, wipe ./.mypy_cache before EVERY timed run: zuban's
  # mypy mode reuses an existing .mypy_cache and has no flag to disable it, so
  # without this its warmed-up runs would be warm incremental hits, not cold.
  # Harmless to the other tools — their caches live in dedicated --cache-dir
  # paths (basilisk-warm/mypy-warm), and cold mypy uses --no-incremental, so
  # none of them read or write ./.mypy_cache. hyperfine excludes --prepare time
  # from the measurement.
  [[ -n "${ZUBAN_PRESENT:-}" ]] && HF+=(--prepare 'rm -rf .mypy_cache')
  for i in "${!TOOL_NAMES[@]}"; do
    name="${TOOL_NAMES[$i]}"
    # Preflight-invalid tool (exit >= 2 — never analyzed this file): not timed.
    # The -warm sibling shares the binary, so it is excluded along with it.
    case "$INVALID_COMBOS" in *" ${STEM}/${name%-warm} "*) continue ;; esac
    CMD="${TOOL_CMDS[$i]//\{\}/$FPATH}"
    HF+=(--command-name "$name" "$CMD")
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
BENCH_COVERAGE="$COVERAGE" \
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

# Preflight coverage: (stem, tool) -> diagnostic count ("" = crashed, no cell).
# Published as <tool>_diags CSV columns so a time is always read next to how
# many diagnostics the tool actually reported on that fixture.
coverage = {}
cov_path = os.environ.get("BENCH_COVERAGE", "")
if cov_path and os.path.exists(cov_path):
    with open(cov_path) as fh:
        for raw in fh:
            parts = raw.rstrip("\n").split("\t")
            if len(parts) == 4:
                coverage[(parts[0], parts[1])] = parts[3]
base_tools = [t for t in all_tools if not t.endswith("-warm")]

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
    f"# note: <tool>_ms = COLD full-file CLI check from scratch (whole process: startup + stubs + analysis). <tool>_diags = error diagnostics the tool reported on that fixture in the measured configuration (error severity only; warnings/notes are not counted) — read every time next to its diags; a tool that reports 0 analyzed the file but flagged no errors there. A blank _ms cell means the tool either was not installed on this machine or failed to analyze that fixture (exit >= 2, e.g. parse abort) and was excluded rather than timed as a crash. Only basilisk and mypy have a -warm column (they keep a real cross-run cache): basilisk-warm = --cache result-cache hit; mypy-warm = incremental .mypy_cache hit (cold mypy = --no-incremental). pyright/ty/pyrefly keep NO cross-run result cache (a repeat run = cold), so they are measured cold-only. zuban is also cold-only but its mypy mode DOES reuse a ./.mypy_cache when present (no flag disables it), so we wipe ./.mypy_cache before every timed run to keep the measurement cold. mypy runs with --strict so it performs the strict-mode analysis the fixtures stress (plain mypy reports 'no issues' on the strictness fixtures); zuban runs as `zuban mypy --strict` for the same reason (its default `zuban check` mode skips these strictness rules).",
    "fixture,"
    + ",".join(f"{t}_ms" for t in all_tools)
    + ","
    + ",".join(f"{t}_diags" for t in base_tools),
]
for stem, means in rows:
    cells = [stem] + [f"{means[t]:.1f}" if t in means else "" for t in all_tools]
    cells += [coverage.get((stem, t), "") for t in base_tools]
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

echo ""
if [[ "${GATE_STATUS:-0}" -ne 0 ]]; then
  echo "RESULT: FAIL — performance regression vs baseline (see gate report above)."
  echo "        Fix the slowdown, or re-baseline with: BENCH_NO_GATE=1 make bench"
else
  echo "RESULT: PASS — no performance regression vs baseline."
  echo "        Commit benchmarks/status/*.csv to track the trend."
fi
exit "${GATE_STATUS:-0}"
