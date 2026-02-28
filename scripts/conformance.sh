#!/usr/bin/env bash
# Run the PEP conformance test suite and print the scorecard.
#
# Downloads the python/typing conformance files first if they are missing.
#
# Usage:
#   ./scripts/conformance.sh           # fetch if needed, then score
#   ./scripts/conformance.sh --fetch   # force re-download, then score

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

CONFORMANCE_DIR="crates/basilisk-cli/tests/conformance"

BOLD='\033[1m'
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RESET='\033[0m'

header() { echo -e "\n${BOLD}${CYAN}▶ $*${RESET}"; }
ok()     { echo -e "${GREEN}✓ $*${RESET}"; }
warn()   { echo -e "${YELLOW}⚠ $*${RESET}"; }

# ── Fetch if missing or forced ────────────────────────────────────────────────

if [[ "${1:-}" == "--fetch" ]] || [[ ! -d "$CONFORMANCE_DIR" ]] || \
   [[ -z "$(ls -A "$CONFORMANCE_DIR" 2>/dev/null)" ]]; then
    header "Fetching conformance suite"
    bash "$REPO_ROOT/scripts/fetch-conformance.sh"
else
    COUNT=$(find "$CONFORMANCE_DIR" -name "*.py" | wc -l | tr -d ' ')
    ok "Conformance suite already present ($COUNT files) — skipping download"
    warn "Use --fetch to force a re-download"
fi

# ── Run the harness ───────────────────────────────────────────────────────────

header "Running PEP conformance harness"
echo ""

cargo test --test conformance_tests -- --nocapture 2>&1

echo ""
header "Done"
echo -e "  See ${CYAN}docs/PEP_CONFORMANCE.md${RESET} for score interpretation and the road to 95%."
echo ""
