#!/usr/bin/env bash
# Run Rust workspace tests with coverage and enforce per-crate thresholds.
#
# Usage:
#   ./scripts/test-rust.sh          # run coverage + thresholds
#   ./scripts/test-rust.sh --open   # open HTML report after

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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

# Ensure llvm-tools-preview is installed so cargo-llvm-cov never prompts.
rustup component add llvm-tools-preview 2>/dev/null || true

# ── Fetch conformance suite if missing ────────────────────────────────────────
CONFORMANCE_DIR="$REPO_ROOT/crates/basilisk-cli/tests/conformance"
if [[ ! -d "$CONFORMANCE_DIR" ]] || [[ -z "$(ls -A "$CONFORMANCE_DIR" 2>/dev/null)" ]]; then
    header "Fetching PEP conformance suite"
    bash "$REPO_ROOT/scripts/conformance.sh" --fetch-only
else
    COUNT=$(find "$CONFORMANCE_DIR" -name "*.py" | wc -l | tr -d ' ')
    ok "Conformance suite already present ($COUNT files)"
fi

# ── Rust tests with coverage ─────────────────────────────────────────────────
# cargo-llvm-cov uses target/llvm-cov-target/ as its target directory,
# so the basilisk binary lands there — not in target/release/.

header "Running tests with coverage instrumentation"
set +e
cargo llvm-cov \
    --profile ci \
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

# Verify the basilisk binary exists.
BASILISK_BIN=$(find_basilisk_bin) || {
    echo -e "${RED}${BOLD}FATAL: basilisk binary not found after coverage build.${RESET}"
    echo -e "${RED}Checked: target/llvm-cov-target/ci/ and fallback paths${RESET}"
    exit 1
}
ok "basilisk binary ready: $BASILISK_BIN"

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
TEST_COVERAGE_BASILISK_CHECKER="${TEST_COVERAGE_BASILISK_CHECKER:-93}"
TEST_COVERAGE_BASILISK_CLI="${TEST_COVERAGE_BASILISK_CLI:-85}"
TEST_COVERAGE_BASILISK_DB="${TEST_COVERAGE_BASILISK_DB:-100}"
TEST_COVERAGE_BASILISK_LSP="${TEST_COVERAGE_BASILISK_LSP:-77}"
TEST_COVERAGE_BASILISK_MOJO="${TEST_COVERAGE_BASILISK_MOJO:-90}"
TEST_COVERAGE_BASILISK_PARSER="${TEST_COVERAGE_BASILISK_PARSER:-100}"
TEST_COVERAGE_BASILISK_PLUGIN="${TEST_COVERAGE_BASILISK_PLUGIN:-100}"
TEST_COVERAGE_BASILISK_RESOLVER="${TEST_COVERAGE_BASILISK_RESOLVER:-94}"
TEST_COVERAGE_BASILISK_STUBS="${TEST_COVERAGE_BASILISK_STUBS:-85}"
TEST_COVERAGE_BASILISK_CONFIG="${TEST_COVERAGE_BASILISK_CONFIG:-93}"
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
