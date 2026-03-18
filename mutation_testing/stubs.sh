#!/usr/bin/env bash
# Mutation testing for basilisk-stubs.
#
# Usage:
#   ./scripts/mutate/stubs.sh

# shellcheck source=_common.sh
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

ensure_cargo_mutants

header "basilisk-stubs mutation testing"

run_mutant_group "basilisk-stubs" "stubs" \
    "." \
    "" \
    ""

print_summary "stubs"
exit $?
