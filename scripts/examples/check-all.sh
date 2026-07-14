#!/usr/bin/env bash
# Run Basilisk against all files in examples/ at once — the violation
# showcases guarantee diagnostics, so a non-zero exit is the success case.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
command -v basilisk >/dev/null || { echo "basilisk not found — install via brew/scoop/pip" >&2; exit 127; }
if basilisk check examples/; then
  echo "Expected diagnostics in examples/, but the check passed." >&2
  exit 1
fi
