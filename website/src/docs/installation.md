---
layout: layouts/docs.njk
title: Installation
description: How to install Basilisk — pre-built binaries, VS Code extension, or build from source.
keywords: basilisk, install, cargo, rust, python type checker, vs code
eleventyNavigation:
  key: Installation
  order: 2
---

# Installation

Basilisk is a single Rust binary with no runtime dependencies. No Node.js. No Python interpreter. No package manager required after installation.

## VS Code extension (recommended)

The fastest way to get started. Install the **Basilisk** extension from the VS Code Marketplace:

1. Open VS Code
2. Go to Extensions (`Ctrl+Shift+X` / `Cmd+Shift+X`)
3. Search for **Basilisk**
4. Click **Install**

**The extension automatically downloads the correct Basilisk binary for your platform** on first activation. No manual setup required. The binary is downloaded from [GitHub Releases](https://github.com/MelbourneDeveloper/Basilisk/releases) and stored in the extension's global storage directory.

### Supported platforms

| OS | Architecture |
|----|-------------|
| macOS | Apple Silicon (aarch64) |
| macOS | Intel (x86_64) |
| Linux | x86_64 |
| Linux | aarch64 |
| Windows | x86_64 |

If the binary is already on your PATH (e.g. from `cargo install`), the extension uses that instead of downloading.

## Pre-built binaries

Download the latest release for your platform from [GitHub Releases](https://github.com/MelbourneDeveloper/Basilisk/releases):

```bash
# macOS (Apple Silicon)
curl -sSfL https://github.com/MelbourneDeveloper/Basilisk/releases/latest/download/basilisk-darwin-aarch64.tar.gz | tar xz
sudo mv basilisk /usr/local/bin/

# macOS (Intel)
curl -sSfL https://github.com/MelbourneDeveloper/Basilisk/releases/latest/download/basilisk-darwin-x86_64.tar.gz | tar xz
sudo mv basilisk /usr/local/bin/

# Linux (x86_64)
curl -sSfL https://github.com/MelbourneDeveloper/Basilisk/releases/latest/download/basilisk-linux-x86_64.tar.gz | tar xz
sudo mv basilisk /usr/local/bin/
```

Verify the installation:

```bash
basilisk --version
```

## Install via Cargo

If you have Rust installed:

```bash
cargo install basilisk
```

This installs the binary to `~/.cargo/bin/`, which is typically already on your PATH if you installed Rust via rustup.

## Build from source

```bash
git clone https://github.com/MelbourneDeveloper/Basilisk
cd Basilisk
cargo build --release
```

The binary is built at `target/release/basilisk`. Add it to your PATH:

```bash
cp target/release/basilisk /usr/local/bin/
```

Rust 1.87+ required.

## CI integration

Basilisk integrates naturally into any CI pipeline. Download the binary in your workflow:

```yaml
# GitHub Actions example
- name: Install Basilisk
  run: |
    curl -sSfL https://github.com/MelbourneDeveloper/Basilisk/releases/latest/download/basilisk-linux-x86_64.tar.gz | tar xz
    sudo mv basilisk /usr/local/bin/

- name: Type check
  run: basilisk check src/
```

Exit codes:
- `0` — No errors found
- `1` — Type errors found
- `2` — Configuration error
- `3` — Internal error

## Editor support (LSP)

Basilisk implements the Language Server Protocol. Any editor with LSP support can use it:

- **VS Code** — via the official Basilisk extension (auto-downloads the binary)
- **Neovim** — via nvim-lspconfig
- **Helix** — native LSP support
- **Zed** — via LSP extension
- **Emacs** — via eglot or lsp-mode

## How the VS Code extension finds the binary

The extension resolves the Basilisk binary in this order:

1. **`basilisk.executablePath` setting** — if you set an explicit path, it's used directly
2. **System PATH** — checks `~/.cargo/bin/`, `/usr/local/bin/`, `/opt/homebrew/bin/`
3. **Extension storage** — checks for a previously downloaded binary
4. **Download prompt** — offers to download the matching version from GitHub Releases
