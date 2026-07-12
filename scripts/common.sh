#!/usr/bin/env bash
# Shared helpers for Basilisk test scripts.
# Source this file — do not execute directly.

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

header() { echo -e "\n${BOLD}${CYAN}▶ $*${RESET}"; }
ok()     { echo -e "${GREEN}✓ $*${RESET}"; }
warn()   { echo -e "${YELLOW}⚠ $*${RESET}"; }

# Coverage threshold for a project, read from coverage-thresholds.json — the
# single source of truth ([COVERAGE-THRESHOLDS-JSON]). No env vars, no
# hardcoded fallbacks. Unknown projects get default_threshold.
coverage_threshold_for() {
    local repo_root
    repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    python3 -c '
import json, sys
data = json.load(open(sys.argv[1]))
project = data["projects"].get(sys.argv[2])
print(project["threshold"] if project else data["default_threshold"])
' "$repo_root/coverage-thresholds.json" "$1"
}

# Locate the basilisk binary. Checks BASILISK_BIN env var first, then known
# build paths. Prints the path on success, returns 1 on failure.
find_basilisk_bin() {
    if [[ -n "${BASILISK_BIN:-}" ]] && [[ -x "$BASILISK_BIN" ]]; then
        echo "$BASILISK_BIN"
        return
    fi
    local repo_root
    repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    for candidate in \
        "$repo_root/target/llvm-cov-target/ci/basilisk" \
        "$repo_root/target/ci/basilisk" \
        "$repo_root/target/llvm-cov-target/release/basilisk" \
        "$repo_root/target/release/basilisk" \
        "$repo_root/target/debug/basilisk"; do
        if [[ -x "$candidate" ]]; then
            echo "$candidate"
            return
        fi
    done
    return 1
}

# Gate a `PlenaryBustedDirectory` run on PARSED test results instead of the nvim
# process exit code. Implements [LSPTEST-EDITOR-SPECIFIC-INTEGRATION-NEOVIM-E2E-GATE]
# (docs/specs/LSP-TEST-INTEGRATION-SPEC.md).
#
# PlenaryBustedDirectory's parent nvim can exit non-zero on teardown — a lingering
# LSP child process or async handle reaped late under parallel CI load (`make ci`
# runs the vsix/nvim/zed suites with `-j3`) — even when every test passed. Gating
# on that exit code therefore produces flaky false failures. It is also too weak in
# the other direction: a run that silently executed no tests would still exit zero.
#
# This gate is strictly stronger. It requires that:
#   1. every `*_spec.lua` under the suite directory STARTED (one `Testing:` line),
#   2. every spec file emitted a final summary (one `Success:` block),
#   3. zero tests failed, zero tests errored, and
#   4. no Lua traceback / nvim runtime error surfaced.
# The nvim exit code is reported for diagnostics but is not authoritative.
#
#   assert_plenary_pass <output_file> <expected_spec_count> <label>
#
# Returns 0 when all four conditions hold, 1 otherwise (printing each problem).
assert_plenary_pass() {
    local out="$1" expected="$2" label="$3"
    local plain started summaries failed errors traces
    # Strip ANSI colour codes so the plenary markers parse cleanly.
    plain="$(sed $'s/\x1b\\[[0-9;]*m//g' "$out")"
    started="$(grep -c '^Testing:' <<<"$plain" || true)"
    summaries="$(grep -c '^Success: ' <<<"$plain" || true)"
    failed="$(awk -F'\t' '/^Failed :/ {s+=$2} END {print s+0}' <<<"$plain")"
    errors="$(awk -F'\t' '/^Errors :/ {s+=$2} END {print s+0}' <<<"$plain")"
    traces="$(grep -cE 'stack traceback:|E5108|Error executing' <<<"$plain" || true)"

    local problems=()
    [[ "$started" -eq "$expected" ]] || problems+=("only ${started}/${expected} spec files started")
    [[ "$summaries" -eq "$expected" ]] || problems+=("only ${summaries}/${expected} spec files produced a summary")
    [[ "$failed" -eq 0 ]] || problems+=("${failed} test(s) failed")
    [[ "$errors" -eq 0 ]] || problems+=("${errors} test(s) errored")
    [[ "$traces" -eq 0 ]] || problems+=("${traces} Lua traceback(s)/runtime error(s) detected")

    if [[ ${#problems[@]} -eq 0 ]]; then
        ok "${label}: ${started}/${expected} files ran, ${summaries} summaries, 0 failed, 0 errored"
        return 0
    fi
    echo -e "  ${RED}${BOLD}✗ ${label} FAILED${RESET}"
    local problem
    for problem in "${problems[@]}"; do
        echo -e "  ${RED}  - ${problem}${RESET}"
    done
    return 1
}

# Classify a `PlenaryBustedDirectory` run for retry purposes, WITHOUT relaxing
# the gate. Echoes exactly one token:
#   pass  — every spec started and summarised, zero failed/errored/tracebacks.
#   flake — the ONLY discrepancy is a short started/summary count while zero
#           tests failed, zero errored and no traceback surfaced. This is the
#           plenary batch-mode flush race: under `make ci`'s `-j3` load a child
#           nvim can exit a beat before flushing its per-file `Success:` footer
#           even though every test in that file passed (observed on the heavy
#           profiler_spec). Safe to re-run.
#   fail  — any real failure: a test failed, a test errored, or a Lua traceback
#           fired. NEVER retried — a genuine regression must fail fast.
#
#   plenary_outcome <output_file> <expected_spec_count>
plenary_outcome() {
    local out="$1" expected="$2" plain started summaries failed errors traces
    plain="$(sed $'s/\x1b\\[[0-9;]*m//g' "$out")"
    started="$(grep -c '^Testing:' <<<"$plain" || true)"
    summaries="$(grep -c '^Success: ' <<<"$plain" || true)"
    failed="$(awk -F'\t' '/^Failed :/ {s+=$2} END {print s+0}' <<<"$plain")"
    errors="$(awk -F'\t' '/^Errors :/ {s+=$2} END {print s+0}' <<<"$plain")"
    traces="$(grep -cE 'stack traceback:|E5108|Error executing' <<<"$plain" || true)"

    # Any real failure is terminal — never a flake.
    if [[ "$failed" -ne 0 || "$errors" -ne 0 || "$traces" -ne 0 ]]; then
        echo "fail"
        return
    fi
    if [[ "$started" -eq "$expected" && "$summaries" -eq "$expected" ]]; then
        echo "pass"
        return
    fi
    # Clean tests, but a spec dropped its footer under load: retryable flush race.
    echo "flake"
}
