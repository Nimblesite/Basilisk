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

# ── Run ───────────────────────────────────────────────────────────────────────
if [[ "$DIFF_MODE" == true ]]; then
    header "Mutation test — changed lines vs origin/$BASE_BRANCH"
    echo -e "  Base: ${CYAN}origin/$BASE_BRANCH${RESET}"
    echo -e "  Only mutants generated from your diff will be tested.\n"
    cargo mutants --jobs 4 --in-diff "origin/$BASE_BRANCH..HEAD"
else
    header "Mutation test — full suite"
    echo -e "  All included crates will be mutated.\n"
    cargo mutants --jobs 4
fi

EXIT=$?

# ── Results ───────────────────────────────────────────────────────────────────
RESULTS_DIR="$REPO_ROOT/mutants.out"

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

exit "$EXIT"
