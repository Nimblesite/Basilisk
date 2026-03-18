#!/usr/bin/env bash
# Mutation testing for basilisk-parser.
#
# NOTE: Historically very few viable mutants — mostly wrapper code around ruff.
#
# Usage:
#   ./scripts/mutate/parser.sh

# shellcheck source=_common.sh
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

ensure_cargo_mutants

header "basilisk-parser mutation testing"

run_mutant_group "basilisk-parser" "parser" \
    "." \
    "" \
    ""

print_summary "parser"
exit $?
