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
