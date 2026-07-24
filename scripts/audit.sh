#!/usr/bin/env bash
# Check all required build/test dependencies.
set -euo pipefail
source "$(dirname "$0")/common.sh"

header "Auditing dependencies"

MISSING=0

require_cmd() {
    if ! command -v "$1" &>/dev/null; then
        echo -e "  ${RED}✗ MISSING: $1 — $2${RESET}"; MISSING=1
    else
        echo -e "  ${GREEN}✓ $1${RESET}"
    fi
}

require_py() {
    if ! python3 -c "import $1" 2>/dev/null; then
        echo -e "  ${RED}✗ MISSING: Python module '$1' — $2${RESET}"; MISSING=1
    else
        echo -e "  ${GREEN}✓ python3 -c 'import $1'${RESET}"
    fi
}

# basilisk.nvim's real floor is Neovim 0.11: lua/basilisk/health.lua hard-errors
# below it and lua/basilisk/lsp.lua drives the 0.11-only vim.lsp.config()/
# vim.lsp.enable(). A presence-only check let an older nvim pass this gate and
# then fail the suite, so ask Neovim itself with the same has() predicate
# health.lua uses. `--clean` keeps a user's config out of the answer.
NVIM_MIN=0.11

require_nvim() {
    if ! command -v nvim &>/dev/null; then
        echo -e "  ${RED}✗ MISSING: nvim — Install Neovim ${NVIM_MIN}+: https://neovim.io${RESET}"; MISSING=1
        return
    fi
    if ! nvim --clean --headless \
        -c "lua io.write(vim.fn.has('nvim-${NVIM_MIN}'))" -c quit 2>/dev/null | grep -q 1; then
        echo -e "  ${RED}✗ TOO OLD: $(nvim --version | head -1) — basilisk.nvim requires Neovim ${NVIM_MIN}+: https://neovim.io${RESET}"
        MISSING=1
        return
    fi
    echo -e "  ${GREEN}✓ nvim (>= ${NVIM_MIN})${RESET}"
}

require_cmd cargo          "Install Rust: https://rustup.rs"
require_cmd cargo-llvm-cov "Install: cargo install cargo-llvm-cov"
require_cmd cargo-audit    "Install: cargo install cargo-audit --locked"
require_cmd node           "Install Node.js 20+: https://nodejs.org"
require_cmd npm            "Bundled with Node.js"
require_cmd python3        "Install Python 3.12: https://python.org"
require_cmd ruff           "Install: pip install ruff"
require_nvim
require_cmd deslop         "Install: scripts/install-deslop.sh"
require_py  debugpy        "Install: pip install debugpy"

if [[ "$MISSING" -ne 0 ]]; then
    echo -e "\n${RED}${BOLD}FATAL: Missing dependencies above.${RESET}"
    exit 1
fi
ok "All dependencies present"
