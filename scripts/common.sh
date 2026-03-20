#!/usr/bin/env bash
# Shared helpers for Basilisk test scripts.
# Source this file — do not execute directly.

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

header() { echo -e "\n${BOLD}${CYAN}▶ $*${RESET}"; }
ok()     { echo -e "${GREEN}✓ $*${RESET}"; }
warn()   { echo -e "${YELLOW}⚠ $*${RESET}"; }

# Locate the basilisk binary. Checks BASILISK_BIN env var first, then known
# build paths. Prints the path on success, returns 1 on failure.
find_basilisk_bin() {
    if [[ -n "${BASILISK_BIN:-}" ]] && [[ -x "$BASILISK_BIN" ]]; then
        echo "$BASILISK_BIN"
        return
    fi
    local repo_root
    repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    for candidate in \
        "$repo_root/target/llvm-cov-target/ci/basilisk" \
        "$repo_root/target/ci/basilisk" \
        "$repo_root/target/llvm-cov-target/release/basilisk" \
        "$repo_root/target/release/basilisk" \
        "$repo_root/target/debug/basilisk"; do
        if [[ -x "$candidate" ]]; then
            echo "$candidate"
            return
        fi
    done
    return 1
}
