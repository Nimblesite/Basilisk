#!/usr/bin/env bash
# Install all dependencies needed to build and test Basilisk.
#
# Usage:
#   ./scripts/setup.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

header() { echo -e "\n${BOLD}${CYAN}▶ $*${RESET}"; }
ok()     { echo -e "${GREEN}✓ $*${RESET}"; }
warn()   { echo -e "${YELLOW}⚠ $*${RESET}"; }
fail()   { echo -e "${RED}✗ $*${RESET}"; exit 1; }

header "Checking required tools"

command -v python3 &>/dev/null || fail "python3 not found"
ok "python3"

command -v rustup &>/dev/null || fail "rustup not found"
ok "rustup"

command -v cargo &>/dev/null || fail "cargo not found"
ok "cargo"

header "Installing Rust toolchain components"

rustup component list --installed | grep -q llvm-tools || rustup component add llvm-tools
ok "llvm-tools"

cargo llvm-cov --version &>/dev/null || cargo install cargo-llvm-cov --locked
ok "cargo-llvm-cov"

cargo audit --version &>/dev/null || cargo install cargo-audit --version 0.22.2 --locked
ok "cargo-audit"

header "Installing Python packages"

python3 -c 'import debugpy' &>/dev/null || python3 -m pip install --quiet --break-system-packages debugpy
ok "debugpy"

# ruff is a hard requirement of `make test` (scripts/audit.sh fails without it)
# and of `make fmt`. Pin it to the version CI installs, which is also the
# ruff_* crate tag in Cargo.toml — a mismatched CLI formats differently from the
# formatter embedded in the binary ([LSPFMT-ENGINE]).
command -v ruff &>/dev/null || python3 -m pip install --quiet --break-system-packages ruff==0.15.17
ok "ruff"

# pytest drives the Neovim e2e harness; scripts/test-nvim.sh aborts without it.
command -v pytest &>/dev/null || python3 -m pip install --quiet --break-system-packages pytest==9.1.1
ok "pytest"

header "Installing Neovim (basilisk.nvim e2e tests)"

# `make test` runs the basilisk.nvim suite and scripts/audit.sh gates on Neovim
# 0.11+ — the floor basilisk.nvim/lua/basilisk/health.lua enforces. Ask Neovim
# itself, with the same has() predicate health.lua uses, so an under-version
# install is upgraded instead of silently accepted.
if nvim --clean --headless -c "lua io.write(vim.fn.has('nvim-0.11'))" -c quit 2>/dev/null | grep -q 1; then
    ok "neovim"
elif command -v brew &>/dev/null; then
    brew install neovim || brew upgrade neovim
    ok "neovim"
else
    fail "Neovim 0.11+ not found and Homebrew is unavailable — install it: https://neovim.io"
fi

header "Installing deslop (duplication gate)"

bash "$REPO_ROOT/scripts/install-deslop.sh"

header "Installing Node dependencies"

if command -v node &>/dev/null && command -v npm &>/dev/null; then
    cd "$REPO_ROOT/vscode-extension"
    npm ci
    ok "vscode-extension node_modules"
    cd "$REPO_ROOT"
else
    warn "node/npm not found — skipping VS Code extension deps"
fi

echo ""
ok "All dependencies installed."
