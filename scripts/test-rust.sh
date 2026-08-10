#!/usr/bin/env bash
# Run Rust workspace tests with coverage and enforce per-crate thresholds.
#
# Usage:
#   ./scripts/test-rust.sh          # run coverage + thresholds
#   ./scripts/test-rust.sh --open   # open HTML report after

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"

# The workflow rewrites shared coverage, mirrored-fixture, and report outputs.
# Serialize whole-worktree runs so a second invocation cannot corrupt the first.
LOCK_PATH="$REPO_ROOT/target/test-rust.lock"
if [[ -z "${BASILISK_TEST_RUST_LOCK_FD:-}" ]] || \
    ! python3 "$REPO_ROOT/conformance/worktree_lock.py" --check "$LOCK_PATH"; then
    mkdir -p "$REPO_ROOT/target"
    exec python3 "$REPO_ROOT/conformance/worktree_lock.py" \
        "$LOCK_PATH" "$0" "$@"
fi

source "$REPO_ROOT/scripts/common.sh"
cd "$REPO_ROOT"

OPEN_REPORT=0
for arg in "$@"; do
    case "$arg" in
        --open) OPEN_REPORT=1 ;;
    esac
done

LCOV_FILE="$REPO_ROOT/lcov.info"
HTML_DIR="$REPO_ROOT/target/llvm-cov/html"
TYPING_SUITE_BASE="$(cd "${TMPDIR:-/tmp}" && pwd -P)"
case "$TYPING_SUITE_BASE/" in
    "$REPO_ROOT/"*)
        echo "FATAL: conformance temp directory must be outside the repository." >&2
        exit 1
        ;;
esac
TYPING_SUITE_PARENT="$(mktemp -d "$TYPING_SUITE_BASE/basilisk-conformance.XXXXXX")"
TYPING_SUITE_DIR="$TYPING_SUITE_PARENT/typing"
export BASILISK_CONFORMANCE_RUN_ID="${TYPING_SUITE_PARENT##*/}"

cleanup_typing_suite() {
    case "$TYPING_SUITE_PARENT" in
        "$TYPING_SUITE_BASE"/basilisk-conformance.??????)
            rm -rf -- "$TYPING_SUITE_PARENT"
            ;;
        *)
            warn "refusing to remove unexpected conformance temp path"
            ;;
    esac
}
trap cleanup_typing_suite EXIT

# Ensure llvm-tools-preview is installed so cargo-llvm-cov never prompts.
rustup component add llvm-tools-preview 2>/dev/null || true

# ── Rust tests + conformance, one instrumented coverage pool ─────────────────
# Coverage is gathered in TWO phases that share ONE profile pool, reported once:
#   1. the workspace test suite, then
#   2. the REAL basilisk binary scored over all 146 PEP conformance fixtures.
# Phase 2 is BOTH the conformance gate AND the source of the checker/resolver
# coverage those files exercise — the compiled binary's own instrumented run
# provides it (there is no in-repo conformance test).
#
# cargo-llvm-cov's `show-env` is the supported way to fold an external binary's
# runs into coverage: source it ONCE, then build + test + run the binary all under
# that single environment so every profraw lands in one pool under target/, then
# report. (Mixing a `cargo llvm-cov <run>` with `show-env` is unsupported — the
# run subcommand redirects to target/llvm-cov-target while show-env uses target/,
# so the pools diverge and the report finds no data.)

header "Running tests with coverage instrumentation"
cargo llvm-cov clean --workspace

# NO clean release build here any more. It existed only to give the conformance
# GATE an un-instrumented binary to score, and that gate is commented out below
# because the measurement cannot run. Building a second full release copy of a
# binary nothing reads cost ~5 minutes of every run.

eval "$(cargo llvm-cov show-env --export-prefix)"

# Coverage-collection fix, EVERY platform. cargo-llvm-cov's default
# `LLVM_PROFILE_FILE` uses the `%Nm` ONLINE-MERGE pattern: every instrumented
# process merges its counters into a shared pool of N files via mmap + POSIX
# advisory file locking. That merge path corrupts under the heavy concurrent
# writes of `cargo test --all-targets`, and is aggravated by the LSP/profiler
# e2e suites, which SIGKILL their instrumented `basilisk` child (see
# basilisk-lsp/tests/common/mod.rs) *mid-merge*. The result is a truncated /
# bad-header `.profraw`, and a single bad header makes `llvm-profdata merge`
# abort the ENTIRE report ("no profile can be merged") — a hard build failure.
#
# This was once believed to be Darwin-only and scoped to macOS "to avoid
# perturbing the green Linux ratchet". Linux disproved it: CI died on
# `Basilisk-<pid>-<hash>_3.profraw: invalid instrumentation profile data (file
# header is corrupt)` — the `_3` names pool slot 3 of the very `%Nm` pool this
# override removes. The kill-mid-write cause is platform-independent; only its
# probability differed, so the workaround belongs on every platform.
#
# Replace the merge pool with ONE plain, independently written profile per
# process (`%p`, no `%m`): no shared pool, no lock contention, no mmap-merge
# corruption. A process that runs to completion writes a valid profile; the ONLY
# files that can still be bad are the SIGKILLed children, which now lose their
# OWN profile instead of corrupting a slot every other process merged into — so
# this preserves strictly MORE coverage than the pool it replaces. The prune step
# below drops those few files before the merge. `cargo llvm-cov report` globs
# every `*.profraw` in the target dir regardless of filename, so dropping `%m`
# does not affect discovery.
export LLVM_PROFILE_FILE="${CARGO_LLVM_COV_TARGET_DIR:-$REPO_ROOT/target}/Basilisk-%p.profraw"

# Sync the (git-ignored) conformance fixtures the Rust tests read, from the REAL
# python/typing suite. `--sync-tests` clones python/typing@main FRESH — done AFTER
# the coverage clean so the clone survives — and mirrors its graded fixtures into
# conformance/tests/ (absent on a fresh checkout, read by e.g. rule_tags_tests'
# `pep_categories_match_conformance_test_prefixes`). The conformance gate below
# reuses this same fresh clone (--reuse-clone) to RUN the harness under this
# instrumented env, so the binary's conformance run also feeds the coverage pool.
header "Syncing PEP conformance fixtures from the real python/typing suite"
python3 -m unittest discover -s "$REPO_ROOT/conformance" -p 'test_*.py'
python3 -m unittest discover -s "$REPO_ROOT/benchmarks" -p 'test_*.py'
python3 "$REPO_ROOT/conformance/run_conformance.py" --suite-dir "$TYPING_SUITE_DIR" --sync-tests

set +e
cargo test --profile ci --workspace --all-targets
TESTS_EXIT=$?
set -e
if [[ "$TESTS_EXIT" -ne 0 ]]; then
    echo ""
    echo -e "${RED}${BOLD}TESTS FAILED (exit $TESTS_EXIT).${RESET}"
    echo -e "${RED}Review the full panic output above. Fix every failure.${RESET}"
    echo -e "${RED}No coverage analysis, no thresholds — NOTHING runs until tests pass.${RESET}"
    exit "$TESTS_EXIT"
fi
ok "All workspace tests passed"

# The freshly-built instrumented binary lives under the show-env build dir
# (target/ci/). Pin BASILISK_BIN to it so the conformance phase scores the exact
# binary whose objects the report reads — not a stale one from another target dir.
export BASILISK_BIN="$REPO_ROOT/target/ci/basilisk"
BASILISK_BIN=$(find_basilisk_bin) || {
    echo -e "${RED}${BOLD}FATAL: instrumented basilisk binary not found after coverage build.${RESET}"
    echo -e "${RED}Checked: target/ci/ and fallback paths${RESET}"
    exit 1
}
ok "instrumented basilisk binary ready: $BASILISK_BIN"

# ── PEP conformance — the REAL python/typing harness, run FRESH ───────────────
# Two passes over the ONE freshly-cloned suite (reused, no re-clone):
#   1. COVERAGE pass — the freshly-built INSTRUMENTED binary checks every fixture
#      under the sourced llvm-cov env, so every `basilisk check` subprocess joins
#      the coverage pool and the checker/resolver paths these fixtures exercise
#      count toward coverage.
#   2. GATE pass — the freshly-built CLEAN RELEASE binary (target/release/basilisk,
#      un-instrumented, exactly what ships) is scored by the REAL harness and MUST
#      hit 100% pass / 0 false positives (coverage-thresholds.json) or the build
#      DIES. run_conformance.py regenerates conformance/conformance_status.csv from
#      the harness's OWN results/basilisk/*.toml on each pass.
# There is NO Rust conformance test, NO vendored calculator, and NO cached
# fixtures: the score is the real suite's own verdict on the CLEAN RELEASE build —
# never an instrumented one, never a prior (PyPI) release. If the real harness
# cannot be cloned and run, this FAILS the build. See [CHKARCH-CONFORMANCE].
# COMMENTED OUT — the measurement cannot run. python/typing no longer registers a
# Basilisk checker, and upstream matches `--only-run` by name against its
# TYPE_CHECKERS tuple, so `--only-run basilisk` grades nothing and writes no
# results/basilisk/*.toml; run_harness() then raises "the real harness wrote no
# results ... it did not run". Both passes below could therefore only ever fail.
# See the conformance._doc note in coverage-thresholds.json. The fixture sync
# above still runs — it only mirrors files and does not invoke the harness.
#
# header "Conformance coverage pass (instrumented binary over the real suite)"
# python3 "$REPO_ROOT/conformance/run_conformance.py" --suite-dir "$TYPING_SUITE_DIR" --bin "$BASILISK_BIN" --reuse-clone
#
# header "Enforcing PEP conformance gate (freshly-built CLEAN RELEASE build vs the REAL harness)"
# python3 "$REPO_ROOT/conformance/run_conformance.py" --suite-dir "$TYPING_SUITE_DIR" --bin "$REPO_ROOT/target/release/basilisk" --gate --reuse-clone

# ── Drop truncated profiles before the merge (every platform) ────────────────
# Completes the `%p` fix above, and runs wherever that does — for the same
# reason: the SIGKILL-mid-write cause is platform-independent, and Linux CI has
# died on exactly this ("no profile can be merged"). All instrumented runs are
# done by now, so any profile `llvm-profdata` cannot read is a `basilisk` child
# the e2e suites SIGKILLed mid-write — a half-written, bad-header file. A single
# unreadable header makes `llvm-profdata merge` (inside `cargo llvm-cov report`)
# abort the WHOLE merge, collapsing every crate to ~0 or failing the build
# outright, so we remove those few files first. Their partial counters carry no
# coverage the cleanly-exited test binaries and the conformance run above don't
# already provide, so the reported number never drops because of this prune.
#
# A prune that silently removed EVERY profile would report ~0% and read as a
# threshold failure rather than the collection failure it is, so an empty pool
# after pruning fails loudly here instead.
header "Pruning truncated coverage profiles"
llvm_profdata="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin/llvm-profdata"
prof_dir="${CARGO_LLVM_COV_TARGET_DIR:-$REPO_ROOT/target}"
if [[ -x "$llvm_profdata" ]]; then
    pruned=0
    kept=0
    while IFS= read -r -d '' profraw; do
        if "$llvm_profdata" show "$profraw" >/dev/null 2>&1; then
            kept=$((kept + 1))
        else
            rm -f "$profraw"
            pruned=$((pruned + 1))
        fi
    done < <(find "$prof_dir" -name '*.profraw' -print0)
    ok "pruned $pruned truncated profile(s) before merge, kept $kept"
    if [[ "$kept" -eq 0 ]]; then
        echo -e "${RED}${BOLD}FATAL: no readable coverage profile survived the prune.${RESET}"
        echo -e "${RED}Every .profraw under $prof_dir was unreadable ($pruned pruned) —${RESET}"
        echo -e "${RED}that is a coverage-COLLECTION failure, not a low-coverage result.${RESET}"
        exit 1
    fi
else
    warn "llvm-profdata not found at $llvm_profdata — skipping prune"
fi

# ── Finalize coverage from BOTH phases (tests + conformance binary runs) ──────
cargo llvm-cov report --profile ci --lcov --output-path "$LCOV_FILE"
ok "lcov.info → $LCOV_FILE"
cargo llvm-cov report --profile ci --html --output-dir "$HTML_DIR"
ok "HTML report → $HTML_DIR/index.html"

# ── Coverage summary ──────────────────────────────────────────────────────────

header "Coverage summary"
REPORT=$(cargo llvm-cov report --profile ci 2>&1)
echo "$REPORT"
echo ""
echo -e "${BOLD}VSCode:${RESET} install 'Coverage Gutters' (ryanluker.vscode-coverage-gutters),"
echo -e "then ${CYAN}Coverage Gutters: Watch${RESET} via Ctrl+Shift+P to see gutter highlights.\n"
[[ "$OPEN_REPORT" -eq 1 ]] && { open "$HTML_DIR/index.html" 2>/dev/null || xdg-open "$HTML_DIR/index.html" 2>/dev/null || true; }

# ── Per-project coverage thresholds ──────────────────────────────────────────

header "Enforcing per-project coverage thresholds"
COV_FAILED=0
HTML_ROWS=""
check_crate() {
    local crate="$1" threshold="$2" totals total_lines missed_lines covered pct
    totals=$(echo "$REPORT" | grep "/${crate}/" | awk '{total+=$8; missed+=$9} END {print total, missed}')
    total_lines=$(echo "$totals" | awk '{print $1}')
    missed_lines=$(echo "$totals" | awk '{print $2}')
    if [ -z "$total_lines" ] || [ "$total_lines" -eq 0 ]; then
        echo -e "  ${RED}✗ ${crate}: NO COVERAGE DATA — tests likely panicked before coverage could flush. FAIL${RESET}"
        COV_FAILED=1
        HTML_ROWS+="<tr class='fail'><td>${crate}</td><td>NO DATA</td><td>${threshold}%</td><td>FAIL</td></tr>"
        return
    fi
    covered=$((total_lines - missed_lines))
    pct=$((covered * 100 / total_lines))
    if [ "$pct" -lt "$threshold" ]; then
        echo -e "  ${RED}✗ ${crate}: ${pct}% < ${threshold}% threshold — FAIL${RESET}"
        COV_FAILED=1
        HTML_ROWS+="<tr class='fail'><td>${crate}</td><td>${pct}% (${covered}/${total_lines})</td><td>${threshold}%</td><td>FAIL</td></tr>"
    else
        echo -e "  ${GREEN}✓ ${crate}: ${pct}% ≥ ${threshold}% threshold${RESET}"
        HTML_ROWS+="<tr class='pass'><td>${crate}</td><td>${pct}% (${covered}/${total_lines})</td><td>${threshold}%</td><td>PASS</td></tr>"
    fi
}
RUST_CRATES=(
    basilisk-checker basilisk-cli basilisk-db basilisk-lsp
    basilisk-parser basilisk-resolver basilisk-stubs basilisk-config
)
for crate in "${RUST_CRATES[@]}"; do
    check_crate "$crate" "$(coverage_threshold_for "$crate")"
done

CRATES_HTML="$HTML_DIR/html/crates.html"
cat > "$CRATES_HTML" <<HTML
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>Basilisk — Crate Coverage Summary</title>
  <style>
    body { font-family: monospace; background: #1e1e1e; color: #d4d4d4; padding: 2rem; }
    h1 { color: #fff; }
    p  { color: #888; }
    a  { color: #4ec9b0; }
    table { border-collapse: collapse; width: 100%; margin-top: 1rem; }
    th { background: #2d2d2d; color: #9cdcfe; padding: 0.5rem 1rem; text-align: left; border-bottom: 2px solid #444; }
    td { padding: 0.4rem 1rem; border-bottom: 1px solid #333; }
    tr.pass td:last-child { color: #4ec9b0; font-weight: bold; }
    tr.fail td:last-child { color: #f44747; font-weight: bold; }
    tr.pass td:nth-child(2) { color: #4ec9b0; }
    tr.fail td:nth-child(2) { color: #f44747; }
  </style>
</head>
<body>
  <h1>Crate Coverage Summary</h1>
  <p>Generated: $(date '+%Y-%m-%d %H:%M') &nbsp;|&nbsp; <a href="index.html">Full file report →</a></p>
  <table>
    <thead><tr><th>Crate</th><th>Line Coverage</th><th>Threshold</th><th>Status</th></tr></thead>
    <tbody>${HTML_ROWS}</tbody>
  </table>
</body>
</html>
HTML
ok "Crate summary → $CRATES_HTML"

if [[ "$COV_FAILED" -ne 0 ]]; then
    echo ""
    echo -e "${RED}Coverage regression detected — one or more projects fell below their threshold.${RESET}"
    exit 1
fi
echo ""
ok "All projects meet their coverage thresholds."
