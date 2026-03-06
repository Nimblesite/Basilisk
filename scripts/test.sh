#!/usr/bin/env bash
# Run the full Basilisk test suite with coverage.
#
# Outputs:
#   lcov.info              — picked up by VSCode Coverage Gutters extension
#   target/llvm-cov/html/  — human-readable HTML report
#
# Usage:
#   ./scripts/test.sh           # run tests + coverage
#   ./scripts/test.sh --open    # also open HTML report in browser

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

header() { echo -e "\n${BOLD}${CYAN}▶ $*${RESET}"; }
ok()     { echo -e "${GREEN}✓ $*${RESET}"; }
warn()   { echo -e "${YELLOW}⚠ $*${RESET}"; }

LCOV_FILE="$REPO_ROOT/lcov.info"
HTML_DIR="$REPO_ROOT/target/llvm-cov/html"

# ── Prerequisites ────────────────────────────────────────────────────────────
header "Checking prerequisites"

if ! rustup component list --installed | grep -q llvm-tools; then
    warn "llvm-tools not installed — installing now"
    rustup component add llvm-tools
fi
ok "llvm-tools present"

if ! cargo llvm-cov --version &>/dev/null; then
    warn "cargo-llvm-cov not found — installing now"
    cargo install cargo-llvm-cov --locked
fi
ok "cargo-llvm-cov present"

# ── Tests + coverage ─────────────────────────────────────────────────────────
header "Running tests with coverage instrumentation (LSP excluded — use scripts/test-lsp.sh)"

# Capture test exit code — intentional Phase-1-limitation failures (E0012,
# E0016-E0019, E0022) panic with "not yet implemented" messages and are
# expected to fail.  We still want coverage data even when they do.
# basilisk-lsp is excluded: its e2e tests require a live LSP process and hang.
TESTS_EXIT=0
cargo llvm-cov \
    --workspace \
    --exclude basilisk-lsp \
    --exclude basilisk-compiler \
    --all-targets \
    --lcov \
    --output-path "$LCOV_FILE" || TESTS_EXIT=$?

ok "lcov.info → $LCOV_FILE"

cargo llvm-cov \
    --workspace \
    --exclude basilisk-lsp \
    --exclude basilisk-compiler \
    --all-targets \
    --html \
    --output-dir "$HTML_DIR" 2>/dev/null || true

ok "HTML report → $HTML_DIR/index.html"

# ── Compiler E2E (passing tests only) ────────────────────────────────────────
header "Running passing compiler E2E tests (hello, arithmetic)"

BASILISK_COMPILER_FILTER="hello,arithmetic" \
    cargo test -p basilisk-compiler --test e2e_tests -- --nocapture
# The full compiler test suite (including expected failures) lives in scripts/test-compiler.sh

# ── Summary ──────────────────────────────────────────────────────────────────
header "Coverage summary"

REPORT=$(cargo llvm-cov report 2>&1)
echo "$REPORT"

echo ""
echo -e "${BOLD}VSCode:${RESET} install 'Coverage Gutters' (ryanluker.vscode-coverage-gutters),"
echo -e "then ${CYAN}Coverage Gutters: Watch${RESET} via Ctrl+Shift+P to see gutter highlights.\n"

if [[ "${1:-}" == "--open" ]]; then
    open "$HTML_DIR/index.html" 2>/dev/null || xdg-open "$HTML_DIR/index.html" 2>/dev/null || true
fi

# ── Per-project coverage thresholds ─────────────────────────────────────────
# Minimum allowed is 85%. Each is set to its current level so coverage never
# regresses. Override via environment variables if needed.
header "Enforcing per-project coverage thresholds"

TEST_COVERAGE_BASILISK_CHECKER="${TEST_COVERAGE_BASILISK_CHECKER:-89}"
TEST_COVERAGE_BASILISK_CLI="${TEST_COVERAGE_BASILISK_CLI:-96}"
TEST_COVERAGE_BASILISK_DB="${TEST_COVERAGE_BASILISK_DB:-100}"
TEST_COVERAGE_BASILISK_LSP="${TEST_COVERAGE_BASILISK_LSP:-0}"
TEST_COVERAGE_BASILISK_MOJO="${TEST_COVERAGE_BASILISK_MOJO:-100}"
TEST_COVERAGE_BASILISK_PARSER="${TEST_COVERAGE_BASILISK_PARSER:-100}"
TEST_COVERAGE_BASILISK_PLUGIN="${TEST_COVERAGE_BASILISK_PLUGIN:-100}"
TEST_COVERAGE_BASILISK_RESOLVER="${TEST_COVERAGE_BASILISK_RESOLVER:-94}"
TEST_COVERAGE_BASILISK_STUBS="${TEST_COVERAGE_BASILISK_STUBS:-100}"

COV_FAILED=0

check_crate() {
    local crate="$1"
    local threshold="$2"

    local totals
    totals=$(echo "$REPORT" | grep "^${crate}/" | awk '{total+=$8; missed+=$9} END {print total, missed}')
    local total_lines missed_lines
    total_lines=$(echo "$totals" | awk '{print $1}')
    missed_lines=$(echo "$totals" | awk '{print $2}')

    if [ -z "$total_lines" ] || [ "$total_lines" -eq 0 ]; then
        warn "${crate}: no coverage data found — skipping"
        return
    fi

    local covered=$((total_lines - missed_lines))
    local pct=$((covered * 100 / total_lines))

    if [ "$pct" -lt "$threshold" ]; then
        echo -e "  ${RED}✗ ${crate}: ${pct}% < ${threshold}% threshold — FAIL${RESET}"
        COV_FAILED=1
    else
        echo -e "  ${GREEN}✓ ${crate}: ${pct}% ≥ ${threshold}% threshold${RESET}"
    fi
}

check_crate basilisk-checker  "$TEST_COVERAGE_BASILISK_CHECKER"
check_crate basilisk-cli      "$TEST_COVERAGE_BASILISK_CLI"
check_crate basilisk-db       "$TEST_COVERAGE_BASILISK_DB"
check_crate basilisk-lsp      "$TEST_COVERAGE_BASILISK_LSP"
check_crate basilisk-mojo     "$TEST_COVERAGE_BASILISK_MOJO"
check_crate basilisk-parser   "$TEST_COVERAGE_BASILISK_PARSER"
check_crate basilisk-plugin   "$TEST_COVERAGE_BASILISK_PLUGIN"
check_crate basilisk-resolver "$TEST_COVERAGE_BASILISK_RESOLVER"
check_crate basilisk-stubs    "$TEST_COVERAGE_BASILISK_STUBS"

# ── Final status ─────────────────────────────────────────────────────────────
if [[ "$TESTS_EXIT" -ne 0 ]]; then
    echo ""
    warn "Some tests failed (exit $TESTS_EXIT)."
    warn "Expected Phase-1 limitation failures: E0012, E0016, E0017, E0018, E0019, E0022"
    warn "These are intentional — they document unimplemented functionality."
    exit "$TESTS_EXIT"
fi

if [[ "$COV_FAILED" -ne 0 ]]; then
    echo ""
    echo -e "${RED}Coverage regression detected — one or more projects fell below their threshold.${RESET}"
    exit 1
fi

echo ""
ok "All projects meet their coverage thresholds."
