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
# WRITE-ALWAYS, GATE-SEPARATELY. Two responsibilities that must NEVER suppress
# each other (benchmarks/summarize.py enforces this):
#
#   1. The measured numbers are written STRAIGHT to the git-tracked status CSV
#      the instant each score exists — after every fixture (incremental) and at
#      the end (final). There is NO path where a run measures a number and does
#      not record it: the file ALWAYS shows what this build actually did. A
#      benchmark that hides a slower number is a lie, and the entire point of the
#      suite is to KNOW the moment a number slips.
#   2. SEPARATELY, and only AFTER the numbers are on disk, the regression gate
#      compares this run's basilisk times against the COMMITTED baseline (the
#      status CSV at HEAD, read from git — never the working copy we just
#      overwrote). A backwards step beyond BENCH_TOLERANCE_PCT on any fixture
#      FAILS CI (non-zero exit). The gate only reads; it never edits the file.
#      Because it reads the committed baseline, overwriting the working copy can
#      never launder a regression into the baseline — the committed baseline
#      advances only when a green run is committed, so it still ratchets faster.
#
# COMPETITOR VERSIONS: every run first pulls the LATEST official release of each
# recognized type checker (see PULL LATEST below), so competitor columns always
# reflect current upstream, never a stale pin.
#
# OUTPUT (auto-generated every run):
#   benchmarks/status/<machine>.csv   — git-tracked per-machine results table,
#                                        ALWAYS rewritten with the latest measured
#                                        numbers (even on a regression). The
#                                        website reads this file, so the published
#                                        numbers are never hand-typed and never
#                                        stale relative to the last run.
#   benchmarks/results/*.json         — raw hyperfine output (untracked)
#   benchmarks/results/summary.md     — human-readable summary (untracked)
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
# Knobs: RUNS=<n> WARMUP=<n>. The gate cannot be DISABLED or WIDENED at runtime
# (BENCH_NO_GATE / BENCH_REGRESS_PCT / BENCH_TOLERANCE_PCT env overrides are
# rejected); the committed tolerance is zero.

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
# The gate cannot be disabled or widened at runtime. The write is unconditional
# (never gated); the committed tolerance is zero so every fixture is
# monotonically non-increasing. It lives in the tracked script and cannot be
# widened away for a run.
if [[ -n "${BENCH_NO_GATE:-}" || -n "${BENCH_REGRESS_PCT:-}" || -n "${BENCH_TOLERANCE_PCT:-}" ]]; then
  echo "ERROR: benchmark regression policy cannot be disabled or widened." >&2
  exit 2
fi
BENCH_GATE="1"
# Zero-tolerance ratchet: any slower fixture is a regression.
BENCH_TOLERANCE_PCT="0"
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
if ! command -v cargo >/dev/null 2>&1; then
  echo "ERROR: cargo is not installed — the benchmark builds a FRESH release binary from this checkout." >&2
  exit 1
fi

# ─── FRESH BINARY: full clean release build from THIS checkout ────────────────
# The benchmark must never time a stale or incrementally-linked binary — a number
# is only honest if it came from a from-scratch optimized build of the exact tree
# under test. So we ALWAYS `cargo clean` the whole target dir and rebuild
# `basilisk --release` before a single fixture is timed. No skip path, no "reuse
# if present" shortcut: the clean build is the first thing that happens, so the
# preflight can never fail on a missing binary — it builds the binary it needs.
echo "─── Fresh release build (full clean, then cargo build --release) ─────────"
if ! cargo clean --manifest-path "$ROOT/Cargo.toml"; then
  echo "ERROR: cargo clean failed — refusing to benchmark a dirty target dir." >&2
  exit 1
fi
if ! cargo build --release --bin basilisk --manifest-path "$ROOT/Cargo.toml"; then
  echo "ERROR: cargo build --release --bin basilisk failed — cannot benchmark." >&2
  exit 1
fi
if [[ ! -x "$BSK" ]]; then
  echo "ERROR: release build reported success but $BSK is missing/not executable." >&2
  exit 1
fi
echo "  fresh binary: $BSK"

# ─── Pull LATEST competitor versions (officially recognized checkers only) ────
# Every run upgrades each recognized type checker to its newest official release
# BEFORE discovery/timing, so competitor columns always reflect current upstream
# and can never publish a stale pin. Only the checkers tracked by the
# python/typing conformance suite are pulled — pyright, mypy, ty, pyrefly, zuban;
# we never add unofficial tools. basilisk itself is the local build under test
# and is never "pulled". The upgrade is best-effort PER TOOL: a failed pull
# (offline, yanked release, non-pip install channel) prints a LOUD warning and
# the run continues on the installed version, so a transient PyPI hiccup can't
# brick the basilisk performance gate — but the staleness is always visible in
# the log, never silent. The pull runs outside all timing, so it never affects a
# measured number. Recognized package names == CLI command names for all five.
RECOGNIZED_CHECKERS="pyright mypy ty pyrefly zuban"
echo "─── Pull latest competitor versions (officially recognized checkers) ────"
if python3 -m pip --version >/dev/null 2>&1; then
  for _tool in $RECOGNIZED_CHECKERS; do
    _before="$(command -v "$_tool" >/dev/null 2>&1 && "$_tool" --version 2>&1 | head -1 || echo 'not installed')"
    if python3 -m pip install --upgrade --quiet --disable-pip-version-check "$_tool" >/dev/null 2>&1; then
      _after="$(command -v "$_tool" >/dev/null 2>&1 && "$_tool" --version 2>&1 | head -1 || echo 'n/a')"
      if [[ "$_before" == "$_after" ]]; then
        printf "  %-9s already latest (%s)\n" "$_tool" "$_after"
      else
        printf "  %-9s %s -> %s\n" "$_tool" "$_before" "$_after"
      fi
    else
      printf "  ⚠ %-9s could not pull latest — using installed (%s). Column may be stale.\n" "$_tool" "$_before" >&2
    fi
  done
else
  echo "  ⚠ python3 -m pip unavailable — cannot pull latest competitors; using installed versions." >&2
fi
echo ""

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

# Clear stale per-fixture JSON from earlier runs so the always-current latest
# CSV (and the final aggregate) only ever reflect THIS run's measurements — a
# fixture removed since last time can no longer leak an old time into either.
rm -f "$OUT"/*.json

# Export the machine/tool metadata ONCE so both the per-fixture incremental
# writer and the final aggregator (benchmarks/summarize.py) see identical
# values. The regression policy stays fixed here — it is never widened.
COVERAGE="$OUT/coverage.tsv"
export BENCH_SLUG BENCH_MACHINE BENCH_CPU BENCH_ARCH BENCH_OS BENCH_CORES \
  BENCH_GENERATED BENCH_TOOLS BENCH_RUNS="$RUNS" BENCH_STATUS_DIR="$STATUS_DIR" \
  BENCH_ALL_TOOLS="$ALL_TOOLS" BENCH_GATE="$BENCH_GATE" \
  BENCH_TOLERANCE_PCT="$BENCH_TOLERANCE_PCT" BENCH_COVERAGE="$COVERAGE" \
  BENCH_ROOT="$ROOT" BENCH_BASELINE_REF="${BENCH_BASELINE_REF:-HEAD}"

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
# COVERAGE path is set + exported above (BENCH_COVERAGE); truncate it fresh here.
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

  # IMMEDIATE WRITE: this fixture's score is now on disk as $OUT/$STEM.json, so
  # rewrite the git-tracked status CSV from every fixture measured so far — the
  # file reflects reality the instant each score exists. Runs BETWEEN timed
  # fixtures (never inside a hyperfine measurement), so it can't perturb timings;
  # if the run dies mid-suite the status CSV already holds every completed score.
  # No gate here — the write is unconditional; the gate runs once, at the end.
  python3 "$ROOT/benchmarks/summarize.py" "$OUT" incremental "${TOOL_NAMES[@]}" >/dev/null || true
done

# ─── Final: console table + summary.md + status CSV (already written) + gate ──
# summarize.py rewrote the status CSV after every fixture; this final call
# re-emits it in full, writes summary.md, prints the table, and runs the
# read-only regression gate against the COMMITTED baseline. The status CSV holds
# this run's real numbers no matter how the gate exits.
echo "─── Summary: mean wall-clock per fixture (ms) ──────────────────────────"
echo ""
python3 "$ROOT/benchmarks/summarize.py" "$OUT" final "${TOOL_NAMES[@]}"
GATE_STATUS=$?

echo ""
if [[ "${GATE_STATUS:-0}" -ne 0 ]]; then
  echo "RESULT: FAIL — performance regression vs the COMMITTED baseline (see gate report above)."
  echo "        The status CSV already holds this run's real numbers — the slip is recorded, not hidden."
  echo "        Optimize the slowdown, then commit benchmarks/status/*.csv to advance the baseline."
else
  echo "RESULT: PASS — no regression vs the committed baseline."
  echo "        The status CSV holds this run's numbers; commit benchmarks/status/*.csv to track the trend."
fi
exit "${GATE_STATUS:-0}"
