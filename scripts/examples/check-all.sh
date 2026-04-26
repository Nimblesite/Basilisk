#!/usr/bin/env bash
# Run Basilisk against all files in examples/ at once.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
cargo run -- check examples/
