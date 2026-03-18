#!/usr/bin/env bash
# Mutation testing for basilisk-config.
#
# Usage:
#   ./scripts/mutate/config.sh

# shellcheck source=_common.sh
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

ensure_cargo_mutants

header "basilisk-config mutation testing"

run_mutant_group "basilisk-config" "config" \
    "." \
    "" \
    ""

print_summary "config"
exit $?
