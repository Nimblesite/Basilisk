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
# Pinned upstream SHA. Bump deliberately, then re-run tests and update
# the conformance threshold in coverage-thresholds.json. Leaving this on
# `main` makes CI non-deterministic — upstream suite changes break us.
TYPING_REF="268d0c4e"
API_URL="https://api.github.com/repos/${TYPING_REPO}/contents/conformance/tests?ref=${TYPING_REF}"

# ── Fetch if missing or forced ───────────────────────────────────────────────
REF_STAMP_FILE="$CONFORMANCE_DIR/.ref-sha"

fetch_conformance() {
    header "Fetching conformance suite from ${TYPING_REPO} (ref: ${TYPING_REF})"
    rm -rf "$CONFORMANCE_DIR"
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

    # The OFFICIAL scorer (conformance/src/main.py) is NOT fetched here — it is
    # committed to the repo at conformance/upstream_main.py (byte-identical,
    # sha256-verified) and score.py imports it directly. To re-pin it after a
    # ref bump: `python3 conformance/score.py --refresh-upstream`.
    echo "$TYPING_REF" > "$REF_STAMP_FILE"
    ok "${COUNT} conformance files written to ${CONFORMANCE_DIR}/ (ref: ${TYPING_REF})"
}

FETCH_ONLY=0
for arg in "$@"; do
    case "$arg" in
        --fetch-only) FETCH_ONLY=1 ;;
    esac
done

CURRENT_REF=""
if [[ -f "$REF_STAMP_FILE" ]]; then
    CURRENT_REF=$(cat "$REF_STAMP_FILE")
fi

# --fetch forces a re-download; --fetch-only only ensures the pinned ref is
# present (fetch if missing/stale, then exit) so `make test` works offline.
if [[ "${1:-}" == "--fetch" ]] || \
   [[ ! -d "$CONFORMANCE_DIR" ]] || \
   [[ -z "$(ls -A "$CONFORMANCE_DIR" 2>/dev/null)" ]] || \
   [[ "$CURRENT_REF" != "$TYPING_REF" ]]; then
    if [[ -n "$CURRENT_REF" ]] && [[ "$CURRENT_REF" != "$TYPING_REF" ]]; then
        warn "Cached conformance ref ($CURRENT_REF) != pinned ($TYPING_REF) — refetching"
    fi
    fetch_conformance
else
    COUNT=$(find "$CONFORMANCE_DIR" -maxdepth 1 -name "*.py" | wc -l | tr -d ' ')
    ok "Conformance suite present ($COUNT files, ref ${TYPING_REF}) — skipping download"
fi

if [[ "$FETCH_ONLY" -eq 1 ]]; then
    exit 0
fi

# ── Score with the OFFICIAL python/typing calculator ─────────────────────────
# We do NOT compute the score ourselves. conformance/score.py imports the
# committed, sha256-verified conformance/upstream_main.py (byte-identical to
# python/typing's own conformance tool at the pinned ref) and runs its real
# get_expected_errors + diff_expected_errors against the actual `basilisk`
# binary. No excluded diagnostic codes; a file passes only with an empty
# upstream errors_diff.
header "Building basilisk binary"
cargo build -p basilisk-cli --bin basilisk

header "Scoring with the official python/typing calculator"
echo ""
python3 conformance/score.py --bin target/debug/basilisk 2>&1

echo ""
header "Done"
echo -e "  Score computed by the REAL python/typing calculator (pinned ${TYPING_REF})."
echo -e "  Per-file results: ${CYAN}conformance/conformance_status.csv${RESET}"
echo ""
