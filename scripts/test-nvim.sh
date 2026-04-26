#!/usr/bin/env bash
# Run Neovim extension real LSP e2e and screenshot regression tests.
#
# Requires: nvim 0.11+, basilisk binary.
# Set BASILISK_BIN to override the binary path.
#
# Usage:
#   ./scripts/test-nvim.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$REPO_ROOT/scripts/common.sh"
cd "$REPO_ROOT"

# Find or build the basilisk binary.
BASILISK_BIN=$(find_basilisk_bin) || {
    header "Building basilisk binary"
    cargo build --profile ci
    BASILISK_BIN="$REPO_ROOT/target/ci/basilisk"
}
if [[ ! -x "$BASILISK_BIN" ]]; then
    echo -e "${RED}${BOLD}FATAL: basilisk binary not found.${RESET}"
    exit 1
fi
export BASILISK_EXECUTABLE_PATH="$BASILISK_BIN"
ok "basilisk binary: $BASILISK_BIN"

# ── Dependencies ──────────────────────────────────────────────────────────────

header "Checking dependencies"

if ! command -v pytest &>/dev/null; then
    echo -e "${RED}${BOLD}FATAL: pytest not found. Install it: pip install pytest${RESET}"
    exit 1
fi
ok "pytest: $(pytest --version 2>&1 | head -1)"

header "Neovim extension — real LSP e2e tests"
cd "$REPO_ROOT/basilisk.nvim"

# Ensure plenary.nvim is available.
if [[ ! -d /tmp/plenary.nvim ]]; then
    git clone --depth 1 https://github.com/nvim-lua/plenary.nvim /tmp/plenary.nvim
fi
# Ensure nvim-dap is available.
if [[ ! -d /tmp/nvim-dap ]]; then
    git clone --depth 1 https://github.com/mfussenegger/nvim-dap /tmp/nvim-dap
fi
# Ensure mini.nvim is available (for screenshot tests).
if [[ ! -d /tmp/mini.nvim ]]; then
    git clone --depth 1 https://github.com/echasnovski/mini.nvim /tmp/mini.nvim
fi

# ── Tests ─────────────────────────────────────────────────────────────────────

if command -v nvim &>/dev/null; then
    # Remove stale luacov data so coverage reflects this run only.
    rm -f luacov.stats.out luacov.report.out

    # Plenary spawns a child nvim per test file. With coverage enabled,
    # children must run sequentially so luacov stats files merge correctly
    # instead of racing on concurrent writes.
    LUACOV=1 nvim --headless -u tests/minimal_init.lua \
        -c "PlenaryBustedDirectory tests/lsp {minimal_init = 'tests/minimal_init.lua', sequential = true}" 2>&1
    ok "Neovim LSP e2e tests passed"

    # Screenshot tests are visual regressions, not coverage inputs. Running
    # them with LUACOV can replace LSP coverage stats with screenshot-only data.
    nvim --headless -u tests/minimal_init.lua \
        -l tests/ui/run_screenshots.lua 2>&1
    ok "Neovim screenshot regression tests passed"
else
    warn "nvim not found — skipping Neovim extension tests"
fi

# ── Coverage threshold (local only — skipped on CI) ──────────────────────────
# luacov records absolute paths which don't match include patterns across
# environments, so coverage enforcement only runs locally.

if [[ -n "${CI:-}" ]]; then
    echo -e "  ${YELLOW:-}⊘ neovim: coverage check skipped on CI${RESET}"
else
    header "Neovim extension — coverage threshold"
    TEST_COVERAGE_NVIM="${TEST_COVERAGE_NVIM:-30}"

    LUACOV=1 nvim --headless -u tests/minimal_init.lua \
        -l tests/run_coverage.lua 2>&1
    ok "Neovim coverage exerciser passed"

    if [[ ! -f luacov.stats.out ]]; then
        echo -e "  ${RED}${BOLD}✗ neovim: no luacov stats — coverage collection is broken. FAIL${RESET}"
        exit 1
    fi

    # Generate coverage report.
    nvim --headless --noplugin -l tests/generate_report.lua 2>&1

    if [[ ! -f luacov.report.out ]]; then
        echo -e "  ${RED}${BOLD}✗ neovim: coverage report generation failed. FAIL${RESET}"
        exit 1
    fi

    # Show summary section.
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
fi
