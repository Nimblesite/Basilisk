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
source "$REPO_ROOT/scripts/common.sh"
cd "$REPO_ROOT"

header "Running Basilisk compiler E2E tests"

cargo test --profile ci -p basilisk-compiler --test e2e_tests -- --nocapture "$@"

ok "All compiler E2E tests passed."
