#!/usr/bin/env bash
# Run LSP and VSIX tests in isolation.
#
# These tests require a live LSP subprocess and take >60s each.
# They are intentionally excluded from scripts/test.sh.
#
# Usage:
#   ./scripts/test-lsp.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$REPO_ROOT/scripts/common.sh"
cd "$REPO_ROOT"

header "Running LSP stdio tests"
cargo test --profile ci -p basilisk-lsp --test lsp_stdio_tests
ok "lsp_stdio_tests done"

header "Running workspace core tests"
cargo test --profile ci -p basilisk-lsp --test ws_core_tests
ok "ws_core_tests done"

header "Running workspace features tests"
cargo test --profile ci -p basilisk-lsp --test ws_features_tests
ok "ws_features_tests done"

header "Running workspace navigation tests"
cargo test --profile ci -p basilisk-lsp --test ws_navigation_tests
ok "ws_navigation_tests done"

header "Running workspace cross-module tests"
cargo test --profile ci -p basilisk-lsp --test ws_test_cross_module
ok "ws_test_cross_module done"

header "Running Zed extension tests"
cargo test --profile ci -p basilisk-lsp --test zed_tests
ok "zed_tests done"
