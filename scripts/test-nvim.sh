#!/usr/bin/env bash
# Run the Neovim plugin specs.
#
# The plugin is a notice ([WITHDRAWAL-SURFACES]): it starts no language server
# and no debug adapter, so this harness needs no `basilisk` binary, no debugpy,
# and no LSP/DAP/screenshot suites. What is left is plenary and one spec
# directory, gated on PARSED results exactly as before — every spec file must
# run AND summarise with zero failures.
#
# Requires: nvim 0.11+.
#
# Usage:
#   ./scripts/test-nvim.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$REPO_ROOT/scripts/common.sh"
cd "$REPO_ROOT"

if ! command -v nvim &>/dev/null; then
    echo -e "${RED}${BOLD}FATAL: nvim not found. Install it: brew install neovim${RESET}" >&2
    exit 1
fi

cd "$REPO_ROOT/basilisk.nvim"

# Test plugins live in /tmp (and are restored from the CI cache), so a directory
# existing proves nothing: macOS's /tmp reaper deletes stale FILES and leaves the
# empty tree behind, and a cache restore can be partial the same way. A hollow
# checkout fails far away from here — `:PlenaryBustedDirectory` simply does not
# exist and every spec is "not an editor command". So the plugin is validated by
# a file it MUST provide and re-cloned when that file is missing.
ensure_plugin() {
    local dir="$1" proof="$2" repo="$3"
    if [[ -f "$dir/$proof" ]]; then
        return 0
    fi
    if [[ -e "$dir" ]]; then
        warn "$dir is present but incomplete (no $proof) — re-cloning"
        rm -rf "$dir"
    fi
    git clone --depth 1 "$repo" "$dir"
    if [[ ! -f "$dir/$proof" ]]; then
        echo -e "${RED}✗ $repo cloned but $proof is missing${RESET}" >&2
        exit 1
    fi
}

ensure_plugin /tmp/plenary.nvim plugin/plenary.vim \
    https://github.com/nvim-lua/plenary.nvim

# Remove stale luacov data so coverage reflects this run only.
rm -f luacov.stats.out luacov.report.out

header "Neovim extension — plugin specs"
expected_specs="$(find tests/basilisk -name '*_spec.lua' | wc -l | tr -d ' ')"
spec_out="$(mktemp)"
set +e
LUACOV=1 nvim --headless -u tests/minimal_init.lua \
    -c "PlenaryBustedDirectory tests/basilisk {minimal_init = 'tests/minimal_init.lua', sequential = true, timeout = 300000}" 2>&1 \
    | tee "$spec_out"
nvim_rc=${PIPESTATUS[0]}
set -e
if [[ "$nvim_rc" -ne 0 ]]; then
    warn "nvim exited ${nvim_rc} after the suite — validating against parsed results (teardown exit is not authoritative)"
fi
if ! assert_plenary_pass "$spec_out" "$expected_specs" "Neovim plugin tests"; then
    rm -f "$spec_out"
    exit 1
fi
rm -f "$spec_out"
ok "Neovim plugin tests passed"

# ── Coverage threshold (local only — skipped on CI) ──────────────────────────
# luacov records absolute paths which don't match include patterns across
# environments, so coverage enforcement only runs locally.

if [[ -n "${CI:-}" ]]; then
    echo -e "  ${YELLOW:-}⊘ neovim: coverage check skipped on CI${RESET}"
    exit 0
fi

header "Neovim extension — coverage threshold"
TEST_COVERAGE_NVIM="$(coverage_threshold_for nvim)"

LUACOV=1 nvim --headless -u tests/minimal_init.lua -l tests/run_coverage.lua 2>&1
ok "Neovim coverage exerciser passed"

if [[ ! -f luacov.stats.out ]]; then
    echo -e "  ${RED}${BOLD}✗ neovim: no luacov stats — coverage collection is broken. FAIL${RESET}"
    exit 1
fi

nvim --headless --noplugin -l tests/generate_report.lua 2>&1

if [[ ! -f luacov.report.out ]]; then
    echo -e "  ${RED}${BOLD}✗ neovim: coverage report generation failed. FAIL${RESET}"
    exit 1
fi

echo "  luacov report summary:"
awk '/^=+$/{s=1} s{print "    "$0}' luacov.report.out | tail -20

# Parse the Total line from the summary: "Total  977  217  81.83%"
nvim_pct=$(awk '/^Total/ { gsub(/%/, "", $NF); printf "%d", $NF }' luacov.report.out)
if [[ -z "$nvim_pct" || "$nvim_pct" -eq 0 ]]; then
    echo -e "  ${RED}${BOLD}✗ neovim: could not parse coverage from luacov report. FAIL${RESET}"
    exit 1
fi

if [[ "$nvim_pct" -lt "$TEST_COVERAGE_NVIM" ]]; then
    echo -e "  ${RED}✗ neovim: ${nvim_pct}% < ${TEST_COVERAGE_NVIM}% threshold — FAIL${RESET}"
    exit 1
fi
echo -e "  ${GREEN}✓ neovim: ${nvim_pct}% ≥ ${TEST_COVERAGE_NVIM}% threshold${RESET}"
