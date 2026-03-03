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
    --all-targets \
    --lcov \
    --output-path "$LCOV_FILE" || TESTS_EXIT=$?

ok "lcov.info → $LCOV_FILE"

cargo llvm-cov \
    --workspace \
    --exclude basilisk-lsp \
    --all-targets \
    --html \
    --output-dir "$HTML_DIR" 2>/dev/null || true

ok "HTML report → $HTML_DIR/index.html"

# ── Summary ──────────────────────────────────────────────────────────────────
header "Coverage summary"

cargo llvm-cov report

echo ""
echo -e "${BOLD}VSCode:${RESET} install 'Coverage Gutters' (ryanluker.vscode-coverage-gutters),"
echo -e "then ${CYAN}Coverage Gutters: Watch${RESET} via Ctrl+Shift+P to see gutter highlights.\n"

if [[ "${1:-}" == "--open" ]]; then
    open "$HTML_DIR/index.html" 2>/dev/null || xdg-open "$HTML_DIR/index.html" 2>/dev/null || true
fi

# ── Final status ─────────────────────────────────────────────────────────────
if [[ "$TESTS_EXIT" -ne 0 ]]; then
    echo ""
    warn "Some tests failed (exit $TESTS_EXIT)."
    warn "Expected Phase-1 limitation failures: E0012, E0016, E0017, E0018, E0019, E0022"
    warn "These are intentional — they document unimplemented functionality."
    exit "$TESTS_EXIT"
fi
