#!/usr/bin/env bash
# Run mutation tests against the Basilisk codebase.
#
# Usage:
#   ./scripts/mutate.sh              # full suite
#   ./scripts/mutate.sh --diff       # only mutate lines changed vs origin/main
#   ./scripts/mutate.sh --diff main  # same, explicit base branch

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
fail()   { echo -e "${RED}✗ $*${RESET}"; }
diag()   { echo -e "${CYAN}  [diag] $*${RESET}" >&2; }

# ── Parse args ───────────────────────────────────────────────────────────────
DIFF_MODE=false
BASE_BRANCH="main"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --diff)
            DIFF_MODE=true
            if [[ "${2:-}" != "" && "${2:-}" != --* ]]; then
                BASE_BRANCH="$2"
                shift
            fi
            shift
            ;;
        *)
            echo "Unknown argument: $1"
            echo "Usage: $0 [--diff [base-branch]]"
            exit 1
            ;;
    esac
done

# ── Prerequisites ─────────────────────────────────────────────────────────────
header "Checking prerequisites"

# 26.2.0 requires rustc 1.88; workspace MSRV is 1.87 — pin to last compatible release.
MUTANTS_VERSION="26.0.0"
if ! cargo mutants --version &>/dev/null; then
    warn "cargo-mutants not found — installing v$MUTANTS_VERSION"
    cargo install "cargo-mutants@$MUTANTS_VERSION" --locked
fi
ok "cargo-mutants $(cargo mutants --version)"

diag "rustc: $(rustc --version)"
diag "cargo: $(cargo --version)"
diag "toolchain: $(rustup show active-toolchain 2>/dev/null || echo 'unknown')"

# ── Sanity-check: clean build MUST pass before handing off to cargo-mutants ──
header "Pre-flight: clean build check"

# Packages with complete, passing test suites — the only ones we mutate.
MUTATE_PACKAGES=(
    basilisk-parser
    basilisk-resolver
    basilisk-checker
    basilisk-cli
)

# Build --package flags
PKG_ARGS=()
for pkg in "${MUTATE_PACKAGES[@]}"; do
    PKG_ARGS+=(--package "$pkg")
done

diag "Wiping stale build artifacts to prevent cargo-mutants cache poisoning..."
cargo clean "${PKG_ARGS[@]}" 2>&1 | sed 's/^/  [clean] /'

diag "Verifying test build compiles cleanly..."
if ! cargo test --no-run "${PKG_ARGS[@]}" 2>&1 | tee /tmp/basilisk-preflight.log | sed 's/^/  [build] /'; then
    fail "PRE-FLIGHT BUILD FAILED — cargo-mutants would fail too. Fix compilation first."
    echo ""
    fail "Full build output:"
    cat /tmp/basilisk-preflight.log
    exit 1
fi
ok "Pre-flight build passed"

diag "Verifying tests actually pass..."
if ! cargo test "${PKG_ARGS[@]}" 2>&1 | tee /tmp/basilisk-tests.log | grep -E "^test result|FAILED|error\[" | sed 's/^/  [test] /'; then
    fail "TESTS FAILED — fix tests before running mutation testing."
    echo ""
    fail "Full test output:"
    cat /tmp/basilisk-tests.log
    exit 1
fi

# Confirm no failures in the test output
if grep -q "FAILED" /tmp/basilisk-tests.log; then
    fail "PRE-FLIGHT TESTS FAILED:"
    grep "FAILED" /tmp/basilisk-tests.log
    exit 1
fi
ok "Pre-flight tests passed"

# ── Run ───────────────────────────────────────────────────────────────────────
if [[ "$DIFF_MODE" == true ]]; then
    header "Mutation test — changed lines vs origin/$BASE_BRANCH"
    echo -e "  Base: ${CYAN}origin/$BASE_BRANCH${RESET}"
    echo -e "  Only mutants generated from your diff will be tested.\n"
    diag "Running: cargo mutants --jobs 4 ${PKG_ARGS[*]} --in-diff origin/$BASE_BRANCH..HEAD"
    cargo mutants --jobs 4 "${PKG_ARGS[@]}" --in-diff "origin/$BASE_BRANCH..HEAD" \
        --output "$REPO_ROOT/mutation_testing/mutants.out" 2>&1 \
        | tee /tmp/basilisk-mutants-run.log
else
    header "Mutation test — full suite"
    echo -e "  Packages: ${MUTATE_PACKAGES[*]}\n"
    diag "Running: cargo mutants --jobs 4 ${PKG_ARGS[*]}"
    cargo mutants --jobs 4 "${PKG_ARGS[@]}" \
        --output "$REPO_ROOT/mutation_testing/mutants.out" 2>&1 \
        | tee /tmp/basilisk-mutants-run.log
fi

EXIT=$?

# ── Diagnose if cargo-mutants itself failed ───────────────────────────────────
if [[ $EXIT -ne 0 ]]; then
    fail "cargo-mutants exited with code $EXIT"
    echo ""
    warn "Scanning mutants run log for errors..."
    grep -E "^error|error\[|FAILED|^ERROR|Failure\(|baseline" /tmp/basilisk-mutants-run.log \
        | sed 's/^/  [mutants-err] /' || true
    echo ""
    warn "Full mutants run log: /tmp/basilisk-mutants-run.log"
fi

# ── Results ───────────────────────────────────────────────────────────────────
RESULTS_DIR="$REPO_ROOT/mutation_testing/mutants.out"

echo ""
header "Results"

if [[ -f "$RESULTS_DIR/missed.txt" && -s "$RESULTS_DIR/missed.txt" ]]; then
    fail "Surviving mutants (tests did NOT catch these):"
    cat "$RESULTS_DIR/missed.txt"
else
    ok "No surviving mutants"
fi

if [[ -f "$RESULTS_DIR/caught.txt" ]]; then
    CAUGHT=$(wc -l < "$RESULTS_DIR/caught.txt" | tr -d ' ')
    ok "Caught: $CAUGHT mutant(s)"
fi

if [[ -f "$RESULTS_DIR/timeout.txt" && -s "$RESULTS_DIR/timeout.txt" ]]; then
    warn "Timed out:"
    cat "$RESULTS_DIR/timeout.txt"
fi

echo ""
echo -e "${BOLD}Full results:${RESET} $RESULTS_DIR/"
echo -e "${BOLD}Mutants run log:${RESET} /tmp/basilisk-mutants-run.log"

# ── HTML report ───────────────────────────────────────────────────────────────
REPORT_SCRIPT="$REPO_ROOT/mutation_testing/mutants_report.py"
OUTCOMES_JSON="$RESULTS_DIR/outcomes.json"
HTML_REPORT="$REPO_ROOT/mutation_testing/mutants_report.html"

if [[ -f "$REPORT_SCRIPT" && -f "$OUTCOMES_JSON" ]]; then
    header "Generating HTML report"
    python3 "$REPORT_SCRIPT" "$OUTCOMES_JSON" "$HTML_REPORT"
    ok "HTML report: $HTML_REPORT"
else
    warn "HTML report skipped (outcomes.json not found)"
fi

exit "$EXIT"
