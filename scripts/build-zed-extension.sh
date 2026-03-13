#!/usr/bin/env bash
# build-zed-extension.sh
#
# Rebuilds the basilisk CLI binary that the Zed extension launches as the LSP.
#
# The Zed extension WASM is compiled by Zed itself when you do:
#   Cmd+Shift+P -> "zed: install dev extension" -> select basilisk-zed/
#
# DO NOT manually copy wasm files into Zed's directories.
# Zed converts raw wasm modules into wasm components — manual copies will fail.
#
# Usage:
#   ./scripts/build-zed-extension.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ZED_DIR="$REPO_ROOT/basilisk-zed"

echo "==> Building basilisk CLI (release)..."
cargo install --path "$REPO_ROOT/crates/basilisk-cli" --force
echo "    Installed: $(which basilisk)"
echo ""
echo "==> CLI binary updated. Now reinstall the dev extension in Zed:"
echo ""
echo "    Cmd+Shift+P -> 'zed: install dev extension'"
echo "    Select: $ZED_DIR"
echo ""
echo "    Zed will recompile the WASM and reload the extension."
