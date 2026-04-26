---
layout: layouts/docs.njk
title: Installation
description: How to install Basilisk — build from source with Cargo, or install the pre-built binary.
keywords: basilisk, install, cargo, rust, python type checker
eleventyNavigation:
  key: Installation
  order: 2
---

# Installation

Basilisk is a single Rust binary with no runtime dependencies. No Node.js. No Python interpreter. No package manager required after installation.

## Requirements

- **Rust 1.87 or later** — [rustup.rs](https://rustup.rs)
- Any operating system: macOS, Linux, Windows

## Build from source

This is the recommended approach during the pre-release phase:

```bash
git clone https://github.com/basilisk-lang/basilisk
cd basilisk
cargo build --release
```

The binary is built at `target/release/basilisk`. Add it to your PATH:

```bash
# macOS / Linux
export PATH="$PATH:$(pwd)/target/release"

# Or copy to a system path
cp target/release/basilisk /usr/local/bin/
```

Verify the installation:

```bash
basilisk --version
```

## Install via Cargo

Once Basilisk is published to crates.io:

```bash
cargo install basilisk
```

This installs the binary to `~/.cargo/bin/`, which is typically already on your PATH if you installed Rust via rustup.

## Run without installing

You can run Basilisk directly from the repository without adding it to PATH:

```bash
cargo run -- check path/to/file.py
cargo run -- check src/
```

## VS Code extension

The Basilisk VS Code extension is in active development (Phase 2). It will provide:

- Real-time diagnostics as you type
- Inline type information on hover
- Quick fixes for every BSK-E and BSK-W code
- Ownership overlay in the gutter
- Type coverage score in the status bar

Track progress at [github.com/basilisk-lang/basilisk](https://github.com/basilisk-lang/basilisk).

## CI integration

Once installed, Basilisk integrates naturally into any CI pipeline:

```yaml
# GitHub Actions example
- name: Type check
  run: basilisk check src/
```

Exit codes:
- `0` — No errors found
- `1` — Type errors found
- `2` — Configuration error
- `3` — Internal error

## Editor support (LSP)

Basilisk implements the Language Server Protocol. Once the LSP server is complete (Phase 2), any editor with LSP support can use it:

- **VS Code** — via the official Basilisk extension
- **Neovim** — via nvim-lspconfig
- **Helix** — native LSP support
- **Zed** — via LSP extension
- **Emacs** — via eglot or lsp-mode

## Updating

To update from source:

```bash
cd basilisk
git pull
cargo build --release
```
