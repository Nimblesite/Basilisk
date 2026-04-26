#!/usr/bin/env bash
# Run Basilisk against examples/bad.py — expects errors.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
cargo run -- check examples/bad.py
