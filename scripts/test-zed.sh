#!/usr/bin/env bash
# Run Zed extension tests.
#
# Usage:
#   ./scripts/test-zed.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$REPO_ROOT/scripts/common.sh"
cd "$REPO_ROOT"

header "Zed extension — tests"
cd "$REPO_ROOT/basilisk-zed"
cargo test --profile ci --all-targets
ok "Zed extension done"
