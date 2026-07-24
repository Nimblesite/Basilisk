#!/usr/bin/env bash
# Run Basilisk against examples/mixed.py — expects some errors, some clean,
# so the check exiting non-zero is the success case.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
command -v basilisk >/dev/null || { echo "basilisk not found — install via brew/scoop/pip" >&2; exit 127; }
if basilisk check examples/mixed.py; then
  echo "Expected diagnostics in examples/mixed.py, but the check passed." >&2
  exit 1
fi
