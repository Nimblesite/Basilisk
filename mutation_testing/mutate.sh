#!/usr/bin/env bash
# Run mutation tests across Basilisk crates.
#
# Usage:
#   ./scripts/mutate.sh                    # run all crates
#   ./scripts/mutate.sh --group fast       # run fast crates only
#   ./scripts/mutate.sh --group checker    # run checker only
#   ./scripts/mutate.sh --group small      # run small crates (stubs, db, config, parser, mojo)
#   ./scripts/mutate.sh --crate stubs      # run a single crate by name
#   ./scripts/mutate.sh --list             # list available crates and groups
#
# Per-crate scripts accept their own flags:
#   ./mutation_testing/checker.sh --group 3  # run checker group 3
#   ./mutation_testing/checker.sh --rule e0014
#   ./mutation_testing/checker.sh --list

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# shellcheck source=_common.sh
source "$SCRIPT_DIR/_common.sh"

# ── Crate definitions ─────────────────────────────────────────────────────────
# Each crate has a script in mutation_testing/<name>.sh
# Groups bundle crates for convenience.

MUTATE_DIR="$REPO_ROOT/mutation_testing"

declare -A CRATE_SCRIPTS
CRATE_SCRIPTS[checker]="$MUTATE_DIR/checker.sh"
CRATE_SCRIPTS[stubs]="$MUTATE_DIR/stubs.sh"
CRATE_SCRIPTS[resolver]="$MUTATE_DIR/resolver.sh"
CRATE_SCRIPTS[db]="$MUTATE_DIR/db.sh"
CRATE_SCRIPTS[config]="$MUTATE_DIR/config.sh"
CRATE_SCRIPTS[parser]="$MUTATE_DIR/parser.sh"
CRATE_SCRIPTS[mojo]="$MUTATE_DIR/mojo.sh"

# Groups: named sets of crates to run together.
# "fast"    — crates that complete in under 5 minutes
# "small"   — small crates (not checker or resolver)
# "checker" — just the checker (the big one)
# "all"     — everything
declare -A MUTATE_GROUPS
MUTATE_GROUPS[fast]="stubs db config parser mojo"
MUTATE_GROUPS[small]="stubs db config parser mojo"
MUTATE_GROUPS[checker]="checker"
MUTATE_GROUPS[resolver]="resolver"
MUTATE_GROUPS[all]="stubs db config parser mojo checker resolver"

SCORES_FILE="$REPO_ROOT/mutation_testing/mutation_scores.csv"

# ── Parse args ────────────────────────────────────────────────────────────────
RUN_GROUP=""
RUN_CRATE=""
LIST_MODE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --group)
            RUN_GROUP="${2:?--group requires a name (fast, small, checker, resolver, all)}"
            shift 2
            ;;
        --crate)
            RUN_CRATE="${2:?--crate requires a name (checker, stubs, db, etc.)}"
            shift 2
            ;;
        --list)
            LIST_MODE=true
            shift
            ;;
        *)
            echo "Unknown argument: $1"
            echo "Usage: $0 [--group NAME] [--crate NAME] [--list]"
            exit 1
            ;;
    esac
done

# ── List mode ─────────────────────────────────────────────────────────────────
if [[ "$LIST_MODE" == true ]]; then
    echo ""
    echo "Available crates:"
    for crate in "${!CRATE_SCRIPTS[@]}"; do
        printf "  %-12s %s\n" "$crate" "${CRATE_SCRIPTS[$crate]}"
    done
    echo ""
    echo "Available groups:"
    for group in "${!MUTATE_GROUPS[@]}"; do
        printf "  %-12s %s\n" "$group" "${MUTATE_GROUPS[$group]}"
    done
    echo ""
    echo "Scores file: $SCORES_FILE"
    echo ""
    echo "Per-crate flags (pass to ./mutation_testing/<crate>.sh):"
    echo "  checker: --group N, --rule eNNNN, --list"
    echo "  others:  (no extra flags)"
    exit 0
fi

# ── Determine which crates to run ─────────────────────────────────────────────
CRATES_TO_RUN=""

if [[ -n "$RUN_CRATE" ]]; then
    if [[ -z "${CRATE_SCRIPTS[$RUN_CRATE]:-}" ]]; then
        fail "Unknown crate: $RUN_CRATE"
        echo "Available: ${!CRATE_SCRIPTS[*]}"
        exit 1
    fi
    CRATES_TO_RUN="$RUN_CRATE"
elif [[ -n "$RUN_GROUP" ]]; then
    if [[ -z "${MUTATE_GROUPS[$RUN_GROUP]:-}" ]]; then
        fail "Unknown group: $RUN_GROUP"
        echo "Available: ${!MUTATE_GROUPS[*]}"
        exit 1
    fi
    CRATES_TO_RUN="${MUTATE_GROUPS[$RUN_GROUP]}"
else
    CRATES_TO_RUN="${MUTATE_GROUPS[all]}"
fi

# ── Run crates ────────────────────────────────────────────────────────────────
header "Mutation testing: $CRATES_TO_RUN"

OVERALL_EXIT=0

for crate in $CRATES_TO_RUN; do
    script="${CRATE_SCRIPTS[$crate]}"
    if [[ ! -x "$script" ]]; then
        chmod +x "$script"
    fi
    header "Running: $crate"
    if "$script"; then
        ok "$crate: done"
    else
        fail "$crate: had surviving mutants"
        OVERALL_EXIT=1
    fi
done

# ── Record scores ─────────────────────────────────────────────────────────────
header "Recording mutation scores"

# Record scores for all crates that have output directories.
for crate in $CRATES_TO_RUN; do
    record_scores "$crate"
done

# ── Overall summary ───────────────────────────────────────────────────────────
echo ""
header "Overall results"
echo -e "${BOLD}Results directory:${RESET} $REPO_ROOT/mutation_testing/"
echo -e "${BOLD}Scores file:${RESET}      $SCORES_FILE"

exit "$OVERALL_EXIT"
