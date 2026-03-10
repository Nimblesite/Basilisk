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
cd "$REPO_ROOT"

GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

header() { echo -e "\n${BOLD}${CYAN}▶ $*${RESET}"; }
ok()     { echo -e "${GREEN}✓ $*${RESET}"; }

header "Running LSP tests"
cargo test -p basilisk-lsp --test lsp_tests
ok "lsp_tests done"

header "Running LSP e2e tests"
cargo test -p basilisk-lsp --test lsp_e2e_tests
ok "lsp_e2e_tests done"
