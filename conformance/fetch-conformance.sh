#!/usr/bin/env bash
# Downloads the python/typing conformance test suite into
# crates/basilisk-cli/tests/conformance/
#
# These files are NOT committed to the repo. Run this script before
# running conformance tests:
#
#   ./conformance/fetch-conformance.sh
#   cargo test --test conformance_tests
#
# The suite is pinned to a specific commit so results are reproducible.
# Update COMMIT below when you want to pull in upstream changes.

set -euo pipefail

REPO="python/typing"
COMMIT="main"   # pin to a tag/SHA for reproducibility, e.g. "ab3c1f2"
API_URL="https://api.github.com/repos/${REPO}/contents/conformance/tests?ref=${COMMIT}"
DEST="$(dirname "$0")/../crates/basilisk-cli/tests/conformance"

mkdir -p "$DEST"

echo "Fetching file list from ${REPO} conformance/tests (ref: ${COMMIT})..."
FILE_LIST=$(curl -fsSL "$API_URL")

COUNT=$(echo "$FILE_LIST" | python3 -c "
import json, sys
files = [f for f in json.load(sys.stdin) if f['type'] == 'file' and f['name'].endswith('.py')]
print(len(files))
")

echo "Downloading ${COUNT} test files to ${DEST}..."

echo "$FILE_LIST" | python3 -c "
import json, sys, urllib.request, os

dest = sys.argv[1]
files = [f for f in json.load(sys.stdin) if f['type'] == 'file' and f['name'].endswith('.py')]

for i, f in enumerate(files, 1):
    out = os.path.join(dest, f['name'])
    urllib.request.urlretrieve(f['download_url'], out)
    if i % 25 == 0 or i == len(files):
        print(f'  {i}/{len(files)}')
" "$DEST"

echo "Done. ${COUNT} conformance files written to ${DEST}/"
