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

# ── Fetch the (git-ignored) conformance fixtures if missing or stale ──────────
# Only the fixtures are downloaded; the official calculator
# (conformance/upstream_main.py) is committed and never fetched. score.py pins
# the ref and re-fetches when the cached ref differs — single source of truth.
header "Ensuring PEP conformance fixtures are current"
python3 "$REPO_ROOT/conformance/score.py" --fetch-only

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

# ── PEP conformance gate ──────────────────────────────────────────────────────
# Score the REAL compiled binary with the official python/typing calculator
# (conformance/score.py imports the committed, sha256-verified upstream_main.py)
# and enforce the ratchet gate from coverage-thresholds.json. This is the whole
# conformance system: two Python files + the gitignored fixtures. No Rust test.
header "Enforcing PEP conformance gate (official python/typing calculator)"
python3 "$REPO_ROOT/conformance/score.py" --bin "$BASILISK_BIN" --gate

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
    basilisk-checker basilisk-cli basilisk-db basilisk-lsp basilisk-mojo
    basilisk-parser basilisk-plugin basilisk-resolver basilisk-stubs basilisk-config
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
