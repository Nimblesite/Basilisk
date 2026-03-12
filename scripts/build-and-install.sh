#!/usr/bin/env bash
# build-and-install.sh
#
# - Cleans Rust and VSIX build artifacts
# - Builds the basilisk binary from scratch (release)
# - Installs it to ~/.cargo/bin (on PATH)
# - Builds the VSIX

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXTENSION_DIR="$REPO_ROOT/vscode-extension"

echo "==> Cleaning Rust build artifacts..."
cargo clean --manifest-path "$REPO_ROOT/Cargo.toml"

echo "==> Cleaning VSIX build artifacts..."
rm -rf "$EXTENSION_DIR/out"
rm -f  "$EXTENSION_DIR"/*.vsix

echo "==> Building basilisk (release)..."
cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml"

echo "==> Installing basilisk to ~/.cargo/bin..."
cargo install --path "$REPO_ROOT/crates/basilisk-cli" --force

echo "==> basilisk installed at: $(which basilisk)"
basilisk --version

echo "==> Building VSIX..."
cd "$EXTENSION_DIR"
npm install
npm run package

VSIX=$(ls "$EXTENSION_DIR"/*.vsix | head -1)

echo "==> Installing VSIX into VS Code..."
code --install-extension "$VSIX" --force

echo ""
echo "==> Done."
echo "    Binary : $(which basilisk)"
echo "    VSIX   : $VSIX"
echo ""
echo "Reload VS Code (Cmd+Shift+P → Developer: Reload Window) to activate."
