#!/usr/bin/env bash
# Mutation testing for basilisk-resolver.
#
# WARNING: Cold builds take 300s+. This crate is expensive to mutate.
#
# Usage:
#   ./scripts/mutate/resolver.sh

# shellcheck source=_common.sh
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

ensure_cargo_mutants

header "basilisk-resolver mutation testing"

run_mutant_group "basilisk-resolver" "resolver" \
    "." \
    "" \
    "--timeout 120 --build-timeout 360"

print_summary "resolver"
exit $?
