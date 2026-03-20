#!/usr/bin/env bash
# Run VS Code extension compile, tests, and coverage threshold.
#
# Expects a basilisk binary to exist (from a prior coverage or cargo build).
# Set BASILISK_BIN to override the binary path.
#
# Usage:
#   ./scripts/test-vscode.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$REPO_ROOT/scripts/common.sh"
cd "$REPO_ROOT"

# Find or build the basilisk binary.
BASILISK_BIN=$(find_basilisk_bin) || {
    header "Building basilisk binary"
    cargo build --profile ci
    BASILISK_BIN="$REPO_ROOT/target/ci/basilisk"
}
if [[ ! -x "$BASILISK_BIN" ]]; then
    echo -e "${RED}${BOLD}FATAL: basilisk binary not found.${RESET}"
    exit 1
fi
ok "basilisk binary: $BASILISK_BIN"

# ── Compile + test ────────────────────────────────────────────────────────────

header "VS Code extension — compile + test"
cd "$REPO_ROOT/vscode-extension"
npm ci
npm run compile
ok "TypeScript compiled"

header "VS Code extension — ESLint"
npm run lint
ok "ESLint passed"

header "VS Code E2E tests"
VSCODE_TEST_CMD="npm test -- --coverage"
# On headless CI (no DISPLAY), wrap with xvfb-run so VS Code can start.
if [[ -z "${DISPLAY:-}" ]] && command -v xvfb-run &>/dev/null; then
    VSCODE_TEST_CMD="xvfb-run -a npm test -- --coverage"
fi
BASILISK_EXECUTABLE_PATH="$BASILISK_BIN" \
MOCHA_TIMEOUT="120000" \
$VSCODE_TEST_CMD
ok "VS Code E2E tests done"

# ── Coverage threshold ────────────────────────────────────────────────────────

header "VS Code extension — coverage threshold"
VSIX_LCOV="$REPO_ROOT/vscode-extension/coverage/lcov.info"
TEST_COVERAGE_VSIX="${TEST_COVERAGE_VSIX:-60}"
if [[ -f "$VSIX_LCOV" ]]; then
    vsix_total=$(grep -c "^DA:" "$VSIX_LCOV" || true)
else
    vsix_total=0
fi
if [[ "$vsix_total" -eq 0 ]]; then
    warn "vscode-extension: no LCOV data — V8 coverage cannot instrument the VS Code extension host process. Skipping threshold."
else
    vsix_covered=$(grep -c "^DA:[^,]*,[^0]" "$VSIX_LCOV" || true)
    vsix_pct=$((vsix_covered * 100 / vsix_total))
    if [[ "$vsix_pct" -lt "$TEST_COVERAGE_VSIX" ]]; then
        echo -e "  ${RED}✗ vscode-extension: ${vsix_pct}% < ${TEST_COVERAGE_VSIX}% threshold — FAIL${RESET}"
        exit 1
    fi
    echo -e "  ${GREEN}✓ vscode-extension: ${vsix_pct}% ≥ ${TEST_COVERAGE_VSIX}% threshold${RESET}"
fi
