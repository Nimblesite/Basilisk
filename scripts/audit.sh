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

require_cmd cargo          "Install Rust: https://rustup.rs"
require_cmd cargo-llvm-cov "Install: cargo install cargo-llvm-cov"
require_cmd node           "Install Node.js 20+: https://nodejs.org"
require_cmd npm            "Bundled with Node.js"
require_cmd python3        "Install Python 3.12: https://python.org"
require_cmd ruff           "Install: pip install ruff"
require_cmd nvim           "Install Neovim 0.10+: https://neovim.io"
require_cmd deslop         "Install: scripts/install-deslop.sh"
require_py  debugpy        "Install: pip install debugpy"

if [[ "$MISSING" -ne 0 ]]; then
    echo -e "\n${RED}${BOLD}FATAL: Missing dependencies above.${RESET}"
    exit 1
fi
ok "All dependencies present"
