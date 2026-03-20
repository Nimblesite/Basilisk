#!/usr/bin/env bash
# Run the full Basilisk test suite.
# Calls individual test scripts in sequence. Each script can also be run standalone.
#
# Usage:
#   ./scripts/test.sh          # run everything
#   ./scripts/test.sh --open   # open HTML coverage report after

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPTS="$REPO_ROOT/scripts"
source "$SCRIPTS/common.sh"

OPEN_FLAG=""
for arg in "$@"; do
    case "$arg" in
        --open) OPEN_FLAG="--open" ;;
    esac
done

# ── Dependency audit ────────────────────────────────────────────────────────
# Every dependency checked up front. Missing anything = immediate hard fail.

header "Auditing dependencies"
MISSING=0
require_cmd() {
    if ! command -v "$1" &>/dev/null; then
        echo -e "  ${RED}✗ MISSING: $1 — $2${RESET}"
        MISSING=1
    else
        echo -e "  ${GREEN}✓ $1${RESET}"
    fi
}
require_py_module() {
    if ! python3 -c "import $1" 2>/dev/null; then
        echo -e "  ${RED}✗ MISSING: Python module '$1' — $2${RESET}"
        MISSING=1
    else
        echo -e "  ${GREEN}✓ python3 -c 'import $1'${RESET}"
    fi
}

require_cmd cargo        "Install Rust: https://rustup.rs"
require_cmd cargo-llvm-cov "Install: cargo install cargo-llvm-cov"
require_cmd node         "Install Node.js 20+: https://nodejs.org"
require_cmd npm          "Bundled with Node.js"
require_cmd python3      "Install Python 3.12: https://python.org"
require_cmd ruff         "Install: pip install ruff"
require_cmd nvim         "Install Neovim 0.10+: https://neovim.io"
require_py_module debugpy "Install: pip install debugpy"

if [[ "$MISSING" -ne 0 ]]; then
    echo ""
    echo -e "${RED}${BOLD}FATAL: Missing dependencies. Install everything listed above, then re-run.${RESET}"
    exit 1
fi
ok "All dependencies present"

# ── Run test scripts ────────────────────────────────────────────────────────

"$SCRIPTS/test-rust.sh" $OPEN_FLAG
"$SCRIPTS/test-vscode.sh"
"$SCRIPTS/test-nvim.sh"
"$SCRIPTS/test-zed.sh"

echo ""
ok "All tests passed."
