#!/usr/bin/env bash
# Mutation testing for basilisk-db.
#
# Usage:
#   ./scripts/mutate/db.sh

# shellcheck source=_common.sh
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

ensure_cargo_mutants

header "basilisk-db mutation testing"

run_mutant_group "basilisk-db" "db" \
    "." \
    "" \
    ""

print_summary "db"
exit $?
