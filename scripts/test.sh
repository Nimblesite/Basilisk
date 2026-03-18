#!/usr/bin/env bash
# Run the full Basilisk test suite with coverage.
#
# Usage:
#   ./scripts/test.sh          # run everything
#   ./scripts/test.sh --open   # open HTML report after

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

OPEN_REPORT=0
for arg in "$@"; do
    case "$arg" in
        --open) OPEN_REPORT=1 ;;
    esac
done

LCOV_FILE="$REPO_ROOT/lcov.info"
HTML_DIR="$REPO_ROOT/target/llvm-cov/html"

# ── Dependency audit ────────────────────────────────────────────────────────
# Every dependency checked up front. Missing anything = immediate hard fail.

header "Auditing dependencies"
MISSING=0
require_cmd() {
    if ! command -v "$1" &>/dev/null; then
        echo -e "  ${RED}✗ MISSING: $1 — $2${RESET}"
        MISSING=1
    else
        echo -e "  ${GREEN}✓ $1${RESET}"
    fi
}
require_py_module() {
    if ! python3 -c "import $1" 2>/dev/null; then
        echo -e "  ${RED}✗ MISSING: Python module '$1' — $2${RESET}"
        MISSING=1
    else
        echo -e "  ${GREEN}✓ python3 -c 'import $1'${RESET}"
    fi
}

require_cmd cargo        "Install Rust: https://rustup.rs"
require_cmd cargo-llvm-cov "Install: cargo install cargo-llvm-cov"
require_cmd node         "Install Node.js 20+: https://nodejs.org"
require_cmd npm          "Bundled with Node.js"
require_cmd python3      "Install Python 3.12: https://python.org"
require_cmd ruff         "Install: pip install ruff"
require_py_module debugpy "Install: pip install debugpy"

if [[ "$MISSING" -ne 0 ]]; then
    echo ""
    echo -e "${RED}${BOLD}FATAL: Missing dependencies. Install everything listed above, then re-run.${RESET}"
    exit 1
fi
ok "All dependencies present"

# Ensure llvm-tools-preview is installed so cargo-llvm-cov never prompts.
rustup component add llvm-tools-preview 2>/dev/null || true

# ── Rust tests with coverage ─────────────────────────────────────────────────
# cargo-llvm-cov uses target/llvm-cov-target/ as its target directory,
# so the basilisk binary lands there — not in target/release/.

header "Running tests with coverage instrumentation"
set +e
cargo llvm-cov \
    --release \
    --workspace \
    --exclude basilisk-compiler \
    --all-targets \
    --lcov \
    --output-path "$LCOV_FILE"
TESTS_EXIT=$?
set -e
ok "lcov.info → $LCOV_FILE"
if [[ "$TESTS_EXIT" -ne 0 ]]; then
    echo ""
    echo -e "${RED}${BOLD}TESTS FAILED (exit $TESTS_EXIT).${RESET}"
    echo -e "${RED}Review the full panic output above. Fix every failure.${RESET}"
    echo -e "${RED}No coverage analysis, no thresholds — NOTHING runs until tests pass.${RESET}"
    exit "$TESTS_EXIT"
fi
ok "All workspace tests passed"

# cargo-llvm-cov places binaries under target/llvm-cov-target/release/.
BASILISK_BIN="$REPO_ROOT/target/llvm-cov-target/release/basilisk"
if [[ ! -x "$BASILISK_BIN" ]]; then
    # Fallback to standard target dir in case cargo-llvm-cov behavior changes.
    BASILISK_BIN="$REPO_ROOT/target/release/basilisk"
fi
if [[ ! -x "$BASILISK_BIN" ]]; then
    echo -e "${RED}${BOLD}FATAL: basilisk binary not found after coverage build.${RESET}"
    echo -e "${RED}Checked: target/llvm-cov-target/release/basilisk and target/release/basilisk${RESET}"
    exit 1
fi
ok "basilisk binary ready: $BASILISK_BIN"

cargo llvm-cov report --release --html --output-dir "$HTML_DIR"
ok "HTML report → $HTML_DIR/index.html"

header "Running passing compiler E2E tests (hello, arithmetic)"
BASILISK_COMPILER_FILTER="hello,arithmetic" \
    cargo test --release -p basilisk-compiler --test e2e_tests -- --nocapture
ok "Compiler E2E tests passed"

header "Coverage summary"
REPORT=$(cargo llvm-cov report --release 2>&1)
echo "$REPORT"
echo ""
echo -e "${BOLD}VSCode:${RESET} install 'Coverage Gutters' (ryanluker.vscode-coverage-gutters),"
echo -e "then ${CYAN}Coverage Gutters: Watch${RESET} via Ctrl+Shift+P to see gutter highlights.\n"
[[ "$OPEN_REPORT" -eq 1 ]] && { open "$HTML_DIR/index.html" 2>/dev/null || xdg-open "$HTML_DIR/index.html" 2>/dev/null || true; }

header "Enforcing per-project coverage thresholds"
TEST_COVERAGE_BASILISK_CHECKER="${TEST_COVERAGE_BASILISK_CHECKER:-89}"
TEST_COVERAGE_BASILISK_CLI="${TEST_COVERAGE_BASILISK_CLI:-96}"
TEST_COVERAGE_BASILISK_DB="${TEST_COVERAGE_BASILISK_DB:-100}"
TEST_COVERAGE_BASILISK_LSP="${TEST_COVERAGE_BASILISK_LSP:-75}"
TEST_COVERAGE_BASILISK_MOJO="${TEST_COVERAGE_BASILISK_MOJO:-90}"
TEST_COVERAGE_BASILISK_PARSER="${TEST_COVERAGE_BASILISK_PARSER:-100}"
TEST_COVERAGE_BASILISK_PLUGIN="${TEST_COVERAGE_BASILISK_PLUGIN:-100}"
TEST_COVERAGE_BASILISK_RESOLVER="${TEST_COVERAGE_BASILISK_RESOLVER:-94}"
TEST_COVERAGE_BASILISK_STUBS="${TEST_COVERAGE_BASILISK_STUBS:-100}"
TEST_COVERAGE_BASILISK_CONFIG="${TEST_COVERAGE_BASILISK_CONFIG:-80}"
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
check_crate basilisk-checker  "$TEST_COVERAGE_BASILISK_CHECKER"
check_crate basilisk-cli      "$TEST_COVERAGE_BASILISK_CLI"
check_crate basilisk-db       "$TEST_COVERAGE_BASILISK_DB"
check_crate basilisk-lsp      "$TEST_COVERAGE_BASILISK_LSP"
check_crate basilisk-mojo     "$TEST_COVERAGE_BASILISK_MOJO"
check_crate basilisk-parser   "$TEST_COVERAGE_BASILISK_PARSER"
check_crate basilisk-plugin   "$TEST_COVERAGE_BASILISK_PLUGIN"
check_crate basilisk-resolver "$TEST_COVERAGE_BASILISK_RESOLVER"
check_crate basilisk-stubs    "$TEST_COVERAGE_BASILISK_STUBS"
check_crate basilisk-config   "$TEST_COVERAGE_BASILISK_CONFIG"

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

# ── LSP tests ────────────────────────────────────────────────────────────────

header "Running LSP tests"
cargo test --release -p basilisk-lsp --test lsp_tests
ok "lsp_tests done"

header "Running LSP E2E tests"
cargo test --release -p basilisk-lsp --test 'lsp_e2e_*'
ok "lsp_e2e tests done"

# ── VS Code extension ────────────────────────────────────────────────────────

header "VS Code extension — compile + test"
cd "$REPO_ROOT/vscode-extension"
npm ci
npm run compile
ok "TypeScript compiled"

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

header "VS Code extension — coverage threshold"
VSIX_LCOV="$REPO_ROOT/vscode-extension/coverage/lcov.info"
TEST_COVERAGE_VSIX="${TEST_COVERAGE_VSIX:-60}"
vsix_total=$(grep -c "^DA:" "$VSIX_LCOV")
vsix_covered=$(grep -c "^DA:[^,]*,[^0]" "$VSIX_LCOV")
vsix_pct=$((vsix_covered * 100 / vsix_total))
if [[ "$vsix_pct" -lt "$TEST_COVERAGE_VSIX" ]]; then
    echo -e "  ${RED}✗ vscode-extension: ${vsix_pct}% < ${TEST_COVERAGE_VSIX}% threshold — FAIL${RESET}"
    exit 1
fi
echo -e "  ${GREEN}✓ vscode-extension: ${vsix_pct}% ≥ ${TEST_COVERAGE_VSIX}% threshold${RESET}"
cd "$REPO_ROOT"

# ── Zed extension ────────────────────────────────────────────────────────────

header "Zed extension — tests"
cd "$REPO_ROOT/basilisk-zed"
cargo test --release --all-targets
ok "Zed extension done"
cd "$REPO_ROOT"

echo ""
ok "All tests passed."
