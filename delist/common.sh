#!/usr/bin/env bash
# Shared helpers for the unlisting scripts.
#
# Implements [WITHDRAWAL-UNLIST]. See
# docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-UNLIST
#
# Every script in this directory removes something from the public internet, so
# they all share one rule: DRY RUN BY DEFAULT. A script prints exactly what it
# would do and changes nothing until it is passed `--yes`. Nothing here is
# reversible by re-running it.

set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'
BOLD='\033[1m'; RESET='\033[0m'

DRY_RUN=1

# Parse the one flag every script accepts. Call with "$@".
parse_args() {
    for arg in "$@"; do
        case "$arg" in
            --yes) DRY_RUN=0 ;;
            --help|-h)
                sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'
                exit 0
                ;;
            *)
                printf "%bunknown argument: %s (only --yes is accepted)%b\n" "$RED" "$arg" "$RESET" >&2
                exit 2
                ;;
        esac
    done
}

step()  { printf "\n%b%b▶ %s%b\n" "$BOLD" "$CYAN" "$1" "$RESET"; }
ok()    { printf "%b✓ %s%b\n" "$GREEN" "$1" "$RESET"; }
warn()  { printf "%b⚠ %s%b\n" "$YELLOW" "$1" "$RESET"; }
fail()  { printf "%b%b✗ %s%b\n" "$BOLD" "$RED" "$1" "$RESET" >&2; exit 1; }

# Run a command, or print it when this is a dry run.
act() {
    if [ "$DRY_RUN" -eq 1 ]; then
        printf "  %bwould run:%b %s\n" "$YELLOW" "$RESET" "$*"
        return 0
    fi
    printf "  %b\$%b %s\n" "$CYAN" "$RESET" "$*"
    "$@"
}

# Refuse to act without a named credential in the environment.
require_env() {
    local name="$1" why="$2"
    if [ -z "${!name:-}" ]; then
        fail "$name is not set — $why"
    fi
}

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || fail "$1 is not installed — $2"
}

# A last human gate in front of an irreversible public change.
confirm() {
    local what="$1"
    if [ "$DRY_RUN" -eq 1 ]; then
        warn "DRY RUN — nothing was changed. Re-run with --yes to $what."
        return 1
    fi
    printf "%b%bAbout to %s. This is public and not undone by re-running.%b\n" \
        "$BOLD" "$YELLOW" "$what" "$RESET"
    printf "Type the word UNLIST to continue: "
    local answer
    read -r answer
    [ "$answer" = "UNLIST" ] || fail "aborted"
    return 0
}

banner() {
    printf "%b%b%s%b\n" "$BOLD" "$CYAN" "$1" "$RESET"
    if [ "$DRY_RUN" -eq 1 ]; then
        warn "dry run — pass --yes to actually make changes"
    fi
}
