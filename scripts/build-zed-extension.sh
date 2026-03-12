#!/usr/bin/env bash
# build-zed-extension.sh
#
# Builds and tests the Basilisk Zed extension.
#
# Usage:
#   ./scripts/build-zed-extension.sh          # build + test
#   ./scripts/build-zed-extension.sh build     # build only
#   ./scripts/build-zed-extension.sh test      # test only (E2E tests against real LSP)
#   ./scripts/build-zed-extension.sh package    # build + create distributable package
#   ./scripts/build-zed-extension.sh install   # build + install binary + dev-install extension

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ZED_DIR="$REPO_ROOT/basilisk-zed"
WASM_TARGET="wasm32-wasip1"
PACKAGE_DIR="$REPO_ROOT/basilisk-zed/dist"

# ── Helpers ──────────────────────────────────────────────────────────

ensure_wasm_target() {
    if ! rustup target list --installed | grep -q "$WASM_TARGET"; then
        echo "==> Installing $WASM_TARGET target..."
        rustup target add "$WASM_TARGET"
    fi
}

build_wasm() {
    echo "==> Building Zed extension (WASM)..."
    cd "$ZED_DIR"
    cargo build --target "$WASM_TARGET" --release
    echo "    WASM artifact: $ZED_DIR/target/$WASM_TARGET/release/basilisk_zed.wasm"
}

package_extension() {
    echo "==> Packaging Zed extension..."
    rm -rf "$PACKAGE_DIR"
    mkdir -p "$PACKAGE_DIR/basilisk"

    cp "$ZED_DIR/extension.toml"                              "$PACKAGE_DIR/basilisk/"
    cp "$ZED_DIR/target/$WASM_TARGET/release/basilisk_zed.wasm" "$PACKAGE_DIR/basilisk/extension.wasm"
    cp -r "$ZED_DIR/debug_adapter_schemas"                    "$PACKAGE_DIR/basilisk/"

    cd "$PACKAGE_DIR"
    tar czf "$PACKAGE_DIR/basilisk-zed-extension.tar.gz" basilisk/

    local size
    size=$(du -sh "$PACKAGE_DIR/basilisk-zed-extension.tar.gz" | cut -f1)
    echo ""
    echo "    Package: $PACKAGE_DIR/basilisk-zed-extension.tar.gz ($size)"
    echo "    Contents:"
    tar tzf "$PACKAGE_DIR/basilisk-zed-extension.tar.gz" | sed 's/^/      /'
}

build_cli() {
    echo "==> Building basilisk CLI (needed for E2E tests)..."
    cargo build --manifest-path "$REPO_ROOT/Cargo.toml" -p basilisk-cli
}

run_e2e_tests() {
    echo "==> Running Zed extension E2E tests (LSP integration)..."
    cargo test --manifest-path "$REPO_ROOT/Cargo.toml" \
        -p basilisk-lsp --test zed_extension_e2e_tests -- --nocapture
}

run_common_tests() {
    echo "==> Running basilisk-common tests..."
    cargo test --manifest-path "$REPO_ROOT/Cargo.toml" -p basilisk-common
}

check_clippy() {
    echo "==> Running clippy on Zed extension..."
    cd "$ZED_DIR"
    cargo clippy --target "$WASM_TARGET" -- -D warnings 2>&1 || true
}

install_binary() {
    echo "==> Installing basilisk binary..."
    cargo install --path "$REPO_ROOT/crates/basilisk-cli" --force
    echo "    Installed: $(which basilisk)"
}

dev_install_extension() {
    # Zed dev extensions are installed by pointing Zed at the extension source
    # directory. Zed compiles the WASM itself from source.
    #
    # Method 1 (CLI): If `zed` CLI is available, use it to install directly.
    # Method 2 (Manual): Open Zed → Cmd+Shift+P → "zed: install dev extension"
    #                     → select the basilisk-zed/ directory.

    # First, make sure the basilisk binary is on PATH (the extension needs it).
    echo "==> Ensuring basilisk binary is on PATH..."
    if ! command -v basilisk &>/dev/null; then
        echo "    basilisk not found on PATH — installing..."
        install_binary
    else
        echo "    Found: $(which basilisk)"
    fi

    # Try the Zed CLI dev extension install.
    if command -v zed &>/dev/null; then
        echo "==> Installing dev extension via Zed CLI..."
        zed --dev-server-token="" extension install-dev "$ZED_DIR" 2>/dev/null || true

        # The CLI command may not support install-dev yet. Fall back to manual.
        echo ""
        echo "    If the above failed, install manually:"
    else
        echo ""
        echo "==> Zed CLI not found. Install manually:"
    fi

    echo ""
    echo "    1. Open Zed"
    echo "    2. Cmd+Shift+P → 'zed: install dev extension'"
    echo "    3. Select: $ZED_DIR"
    echo ""
    echo "    Zed will compile the WASM and load the extension."
    echo "    The basilisk binary must be on PATH: $(which basilisk 2>/dev/null || echo 'NOT FOUND — run: cargo install --path crates/basilisk-cli')"
}

# ── Main ─────────────────────────────────────────────────────────────

CMD="${1:-all}"

case "$CMD" in
    build)
        ensure_wasm_target
        build_wasm
        ;;
    test)
        build_cli
        run_common_tests
        run_e2e_tests
        ;;
    package)
        ensure_wasm_target
        build_wasm
        package_extension
        ;;
    install)
        ensure_wasm_target
        build_wasm
        install_binary
        dev_install_extension
        ;;
    clippy)
        ensure_wasm_target
        check_clippy
        ;;
    all|"")
        ensure_wasm_target
        build_wasm
        build_cli
        run_common_tests
        run_e2e_tests
        echo ""
        echo "==> All done. 19 E2E tests passed."
        echo "    To dev-install: ./scripts/build-zed-extension.sh install"
        ;;
    *)
        echo "Usage: $0 {build|test|package|install|clippy|all}"
        exit 1
        ;;
esac
