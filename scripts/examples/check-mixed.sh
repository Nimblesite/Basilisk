#!/usr/bin/env bash
# Run Basilisk against examples/mixed.py — expects some errors, some clean.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
cargo run -- check examples/mixed.py
