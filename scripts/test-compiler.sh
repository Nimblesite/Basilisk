#!/usr/bin/env bash
# Run the Basilisk compiler E2E tests.
#
# Compiles each .py fixture, runs it, and asserts output matches expected.
#
# Usage:
#   ./scripts/test-compiler.sh              # run all compiler e2e tests
#   ./scripts/test-compiler.sh --nocapture  # show stdout/stderr from tests

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

header() { echo -e "\n${BOLD}${CYAN}▶ $*${RESET}"; }
ok()     { echo -e "${GREEN}✓ $*${RESET}"; }

header "Running Basilisk compiler E2E tests"

cargo test -p basilisk-compiler --test e2e_tests -- --nocapture "$@"

ok "All compiler E2E tests passed."
