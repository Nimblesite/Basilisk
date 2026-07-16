#!/usr/bin/env bash
# Run Basilisk against examples/good.py — expects a clean pass.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
command -v basilisk >/dev/null || { echo "basilisk not found — install via brew/scoop/pip" >&2; exit 127; }
basilisk check examples/good.py
