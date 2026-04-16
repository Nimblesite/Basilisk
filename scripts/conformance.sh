#!/usr/bin/env bash
# Run the PEP conformance test suite.
#
# Downloads the python/typing conformance files first if they are missing.
# Outputs: conformance/conformance_status.csv (committed to the repo).
#
# Usage:
#   ./scripts/conformance.sh              # fetch if needed, then score
#   ./scripts/conformance.sh --fetch      # force re-download, then score
#   ./scripts/conformance.sh --fetch-only # fetch only, no test run

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$REPO_ROOT/scripts/common.sh"
cd "$REPO_ROOT"

CONFORMANCE_DIR="crates/basilisk-cli/tests/conformance"

# ── Fetch configuration ──────────────────────────────────────────────────────
TYPING_REPO="python/typing"
TYPING_REF="main"   # pin to a tag/SHA for reproducibility
API_URL="https://api.github.com/repos/${TYPING_REPO}/contents/conformance/tests?ref=${TYPING_REF}"

# ── Fetch if missing or forced ───────────────────────────────────────────────
fetch_conformance() {
    header "Fetching conformance suite from ${TYPING_REPO} (ref: ${TYPING_REF})"
    mkdir -p "$CONFORMANCE_DIR"

    CURL_ARGS=(-fsSL)
    if [[ -n "${GITHUB_TOKEN:-}" ]]; then
        CURL_ARGS+=(-H "Authorization: token ${GITHUB_TOKEN}")
    fi
    FILE_LIST=$(curl "${CURL_ARGS[@]}" "$API_URL")

    COUNT=$(echo "$FILE_LIST" | python3 -c "
import json, sys
files = [f for f in json.load(sys.stdin) if f['type'] == 'file' and f['name'].endswith('.py')]
print(len(files))
")

    echo "Downloading ${COUNT} test files to ${CONFORMANCE_DIR}..."

    echo "$FILE_LIST" | python3 -c "
import json, sys, urllib.request, os

dest = sys.argv[1]
files = [f for f in json.load(sys.stdin) if f['type'] == 'file' and f['name'].endswith('.py')]

for i, f in enumerate(files, 1):
    out = os.path.join(dest, f['name'])
    urllib.request.urlretrieve(f['download_url'], out)
    if i % 25 == 0 or i == len(files):
        print(f'  {i}/{len(files)}')
" "$CONFORMANCE_DIR"

    ok "${COUNT} conformance files written to ${CONFORMANCE_DIR}/"
}

FETCH_ONLY=0
for arg in "$@"; do
    case "$arg" in
        --fetch-only) FETCH_ONLY=1 ;;
    esac
done

if [[ "${1:-}" == "--fetch" ]] || [[ "${1:-}" == "--fetch-only" ]] || \
   [[ ! -d "$CONFORMANCE_DIR" ]] || \
   [[ -z "$(ls -A "$CONFORMANCE_DIR" 2>/dev/null)" ]]; then
    fetch_conformance
else
    COUNT=$(find "$CONFORMANCE_DIR" -name "*.py" | wc -l | tr -d ' ')
    ok "Conformance suite already present ($COUNT files) — skipping download"
    warn "Use --fetch to force a re-download"
fi

if [[ "$FETCH_ONLY" -eq 1 ]]; then
    exit 0
fi

# ── Run the harness ──────────────────────────────────────────────────────────
header "Running PEP conformance harness"
echo ""

cargo test --test conformance_tests -- --nocapture 2>&1

echo ""
header "Done"
echo -e "  See ${CYAN}docs/PEP_CONFORMANCE.md${RESET} for score interpretation and the road to 95%."
echo ""
