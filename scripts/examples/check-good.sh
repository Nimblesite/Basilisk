#!/usr/bin/env bash
# Run Basilisk against examples/good.py — expects a clean pass.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
cargo run -- check examples/good.py
