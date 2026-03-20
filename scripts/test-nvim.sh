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
    nvim --headless -u tests/minimal_init.lua \
        -c "PlenaryBustedDirectory tests/lsp {minimal_init = 'tests/minimal_init.lua'}" 2>&1
    ok "Neovim LSP e2e tests passed"

    nvim --headless -u tests/minimal_init.lua \
        -l tests/ui/run_screenshots.lua 2>&1
    ok "Neovim screenshot regression tests passed"
else
    warn "nvim not found — skipping Neovim extension tests"
fi
