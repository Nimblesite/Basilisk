---
layout: layouts/docs.njk
title: "Install Basilisk — VS Code, Cursor, Zed, Neovim, or CLI"
description: "Install the Basilisk Python language server for your editor — VS Code, Cursor, Windsurf, Zed, or Neovim — or as a standalone CLI via Homebrew, Scoop, or pre-built binaries. Single Rust binary, no runtime dependencies."
keywords: basilisk, install, vs code, cursor, windsurf, zed, neovim, homebrew, scoop, open vsx, python language server, rust
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Installation
  order: 2
---

# Installation

Basilisk is a single Rust binary with no runtime dependencies — no Node.js, no Python interpreter, no package manager required after installation. **For every supported editor, the binary comes with the extension; you never install it separately.**

Pick your setup:

| If you use… | Install guide | The binary is… |
|---|---|---|
| **VS Code, Cursor, Windsurf** | [VS Code & Cursor](/docs/install-vscode/) | bundled inside the extension |
| **Zed** | [Zed](/docs/install-zed/) | downloaded with the extension on first run |
| **The command line / CI** | [CLI & Package Managers](/docs/install-cli/) | installed via Homebrew, Scoop, or a release binary |

## Editor support (LSP)

Basilisk implements the Language Server Protocol, so any LSP-capable editor can use it:

- **VS Code** — official extension, binary bundled → [guide](/docs/install-vscode/)
- **Cursor, Windsurf & other VS Code forks** — via [Open VSX](https://open-vsx.org) → [guide](/docs/install-vscode/)
- **Zed** — native extension, binary auto-downloaded → [guide](/docs/install-zed/)
- **Neovim** — via the `basilisk.nvim` plugin (auto-downloads the binary)
- **Helix** — native LSP support (point it at a [CLI install](/docs/install-cli/))
- **Emacs** — via eglot or lsp-mode (point it at a [CLI install](/docs/install-cli/))
- **JetBrains (IntelliJ / PyCharm)** — coming soon

## Next steps

Once installed, head to the [Quick Start](/docs/quick-start/) to run your first type check, or the [Configuration](/docs/configuration/) reference to tune Basilisk in `pyproject.toml`.
