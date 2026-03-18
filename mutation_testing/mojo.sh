#!/usr/bin/env bash
# Mutation testing for basilisk-mojo.
#
# Usage:
#   ./scripts/mutate/mojo.sh

# shellcheck source=_common.sh
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

ensure_cargo_mutants

header "basilisk-mojo mutation testing"

run_mutant_group "basilisk-mojo" "mojo" \
    "." \
    "" \
    ""

print_summary "mojo"
exit $?
