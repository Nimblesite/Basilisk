#!/usr/bin/env bash
# Run Basilisk against examples/bad.py — expects diagnostics, so the check
# exiting non-zero is the success case.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
command -v basilisk >/dev/null || { echo "basilisk not found — install via brew/scoop/pip" >&2; exit 127; }
if basilisk check examples/bad.py; then
  echo "Expected diagnostics in examples/bad.py, but the check passed." >&2
  exit 1
fi
