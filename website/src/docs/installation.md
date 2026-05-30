---
layout: layouts/docs.njk
title: "Install Basilisk — VS Code, Cursor, Zed, Neovim, or CLI"
description: "Install the Basilisk Python language server via the VS Code Marketplace, Homebrew, Scoop, pre-built binaries, or build from source. Supports macOS, Linux, and Windows."
keywords: basilisk, install, open vsx, homebrew, scoop, rust, python language server, vs code, cursor, windsurf, zed, neovim
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Installation
  order: 2
---

# Installation

Basilisk is a single Rust binary with no runtime dependencies. No Node.js. No Python interpreter. No package manager required after installation.

## VS Code (recommended)

The fastest way to get started. Install the **Basilisk** extension from your editor's marketplace:

1. Open your editor
2. Go to Extensions (`Ctrl+Shift+X` / `Cmd+Shift+X`)
3. Search for **Basilisk**
4. Click **Install**

The extension is published to the **VS Code Marketplace** today. Publishing to **[Open VSX](https://open-vsx.org)** — which makes it installable in **Cursor**, **Windsurf**, and other VS Code-compatible editors — is **coming very soon**.

**The extension bundles the matching Basilisk binary for your platform.** No manual setup is required for a default VSIX install.

```bash
git clone https://github.com/Nimblesite/Basilisk
cd basilisk
cargo build --release
```

| OS | Architecture |
|----|-------------|
| macOS | Apple Silicon (aarch64) |
| macOS | Intel (x86_64) |
| Linux | x86_64 |
| Linux | aarch64 |
| Windows | x86_64 |

Use `basilisk.executablePath`, `basilisk.binaries.basilisk`, or `basilisk.binaries.path` only when you intentionally want to override the bundled VSIX binary.

## Homebrew (macOS, Linux)

```bash
brew tap Nimblesite/tap
brew install basilisk
```

This installs the latest released `basilisk` binary on macOS (Apple Silicon) and Linux (x86_64, aarch64). Upgrade with `brew upgrade basilisk`.

## Scoop (Windows)

```powershell
scoop bucket add nimblesite https://github.com/Nimblesite/scoop-bucket
scoop install basilisk
```

This installs the latest released `basilisk.exe` on Windows (x86_64 and arm64). Upgrade with `scoop update basilisk`.

## Pre-built binaries

Download the latest release for your platform from [GitHub Releases](https://github.com/Nimblesite/Basilisk/releases):

```bash
# macOS (Apple Silicon)
curl -sSfL -o basilisk.zip https://github.com/Nimblesite/Basilisk/releases/latest/download/basilisk-aarch64-apple-darwin.zip
unzip basilisk.zip && sudo mv basilisk /usr/local/bin/

# Linux (x86_64)
curl -sSfL https://github.com/Nimblesite/Basilisk/releases/latest/download/basilisk-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv basilisk /usr/local/bin/

# Linux (aarch64)
curl -sSfL https://github.com/Nimblesite/Basilisk/releases/latest/download/basilisk-aarch64-unknown-linux-gnu.tar.gz | tar xz
sudo mv basilisk /usr/local/bin/
```

Verify the installation:

```bash
basilisk --version
```

## Build from source

```bash
git clone https://github.com/Nimblesite/Basilisk
cd Basilisk
cargo build --release
```

The binary is built at `target/release/basilisk`. Add it to your PATH:

```bash
cp target/release/basilisk /usr/local/bin/
```

Rust 1.87+ required.

Track progress at [github.com/Nimblesite/Basilisk](https://github.com/Nimblesite/Basilisk).

## CI integration

Basilisk integrates naturally into any CI pipeline. Download the binary in your workflow:

```yaml
# GitHub Actions example
- name: Install Basilisk
  run: |
    curl -sSfL https://github.com/Nimblesite/Basilisk/releases/latest/download/basilisk-x86_64-unknown-linux-gnu.tar.gz | tar xz
    sudo mv basilisk /usr/local/bin/

- name: Type check
  run: basilisk check src/
```

Exit codes:
- `0` — No errors found
- `1` — Type errors found
- `2` — Configuration error
- `3` — Internal error

## Zed extension

Basilisk provides a native Zed extension that registers the LSP for Python files. When installed, Basilisk automatically activates as the language server for all `.py` files.

### Install the extension

1. Build and install the Basilisk CLI binary:

```bash
cargo install --path crates/basilisk-cli
```

This installs the binary to `~/.cargo/bin/basilisk`.

2. Install the dev extension in Zed:

- Open the command palette: `Cmd+Shift+P`
- Run **zed: install dev extension**
- Select the `basilisk-zed/` directory from the repository

Zed compiles the extension's Rust source to WASM automatically. You do not need to pre-build or copy any `.wasm` files.

3. Open a Python file — Basilisk is now your Python language server.

### How Zed finds the binary

The Zed extension resolves the Basilisk binary in this order:

1. **Zed LSP settings** — if you configure an explicit path in your Zed `settings.json`:

```json
{
  "lsp": {
    "basilisk": {
      "binary": {
        "path": "/path/to/basilisk"
      }
    }
  }
}
```

2. **`BASILISK_PATH` environment variable** — set this to override the default location
3. **`~/.cargo/bin/basilisk`** — the default location where `cargo install` places the binary

Zed does **not** resolve bare command names from PATH. The extension always returns an absolute path to the binary.

### Configure Basilisk settings in Zed

Add Basilisk-specific settings to your Zed `settings.json`:

```json
{
  "lsp": {
    "basilisk": {
      "settings": {
        "analysisMode": "wholeModule"
      }
    }
  }
}
```

> The language server currently honors `analysisMode` (`wholeModule` or
> `openFilesOnly`) and the `testExplorer` settings. Other settings are not yet
> read by the server — see the [configuration reference](/docs/configuration/)
> for what is actually wired up today.

### Rebuilding after changes

If you modify the Basilisk source:

1. Rebuild the CLI binary: `cargo install --path crates/basilisk-cli --force`
2. Reinstall the dev extension in Zed: `Cmd+Shift+P` → **zed: install dev extension** → select `basilisk-zed/`

Zed recompiles the WASM and reloads the extension automatically.

## Editor support (LSP)

Basilisk implements the Language Server Protocol. Any editor with LSP support can use it:

- **VS Code** — via the official Basilisk extension (bundles the matching binary)
- **Cursor, Windsurf & other VS Code forks** — via [Open VSX](https://open-vsx.org) (coming very soon)
- **Zed** — via the Basilisk Zed extension (see above)
- **Neovim** — via nvim-lspconfig
- **Helix** — native LSP support
- **Emacs** — via eglot or lsp-mode
- **JetBrains (IntelliJ / PyCharm)** — coming soon

## How the VS Code extension finds the binary

The extension resolves the Basilisk binary in this order:

1. **Explicit component path** — `basilisk.binaries.basilisk` or `basilisk.executablePath`
2. **Explicit binary directory** — `basilisk.binaries.path`
3. **Bundled VSIX binary** — `bin/<platform>/basilisk`
4. **External install** — Cargo, Homebrew, Scoop, or PATH if the version matches

Homebrew and Scoop are external override or repair sources. A default VSIX install runs the binary bundled inside the VSIX.
