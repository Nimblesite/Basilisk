---
layout: layouts/docs.njk
title: "Install Basilisk — VS Code, Cursor, Zed, Neovim, or CLI"
description: "Install the Basilisk Python language server for your editor — VS Code, Cursor, Windsurf, Zed, or Neovim — or as a standalone CLI via PyPI (uv tool install or pipx), Homebrew, Scoop, or pre-built binaries. Single Rust binary, no runtime dependencies."
keywords: basilisk, install, vs code, cursor, windsurf, zed, neovim, pypi, pip, uv, pipx, homebrew, scoop, open vsx, python language server, rust
date: 2026-02-28
dateModified: 2026-07-19
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
| **Neovim** | [Neovim](/docs/install-neovim/) | downloaded by the plugin on first use |
| **The command line / CI** | [CLI & Package Managers](/docs/install-cli/) | installed via PyPI (`uv tool install`), Homebrew, Scoop, or a release binary |

## Editor support (LSP)

Basilisk implements the Language Server Protocol, so any LSP-capable editor can use it:

- **VS Code** — official extension, binary bundled → [guide](/docs/install-vscode/)
- **Cursor, Windsurf & other VS Code forks** — via [Open VSX](https://open-vsx.org) → [guide](/docs/install-vscode/)
- **Zed** — native extension, binary auto-downloaded → [guide](/docs/install-zed/)
- **Neovim** — official `basilisk.nvim` plugin, binary auto-downloaded → [guide](/docs/install-neovim/)
- **Helix** — native LSP support (point it at a [CLI install](/docs/install-cli/))
- **Emacs** — via eglot or lsp-mode (point it at a [CLI install](/docs/install-cli/))
- **JetBrains (IntelliJ / PyCharm)** — coming soon

## Integration status by editor

Where the full Basilisk workflow stands in each editor today — ✅ shipped, 🌗 partial, ⛔️ not yet:

| IDE                            | Est. users | Deployed | LSP | Format | Profiling | Memory | Debugging | Testing | MCP |
|--------------------------------|:----------:|:--------:|:---:|:------:|:---------:|:------:|:---------:|:-------:|:---:|
| VS Code                        | [50M MAU](https://developer.microsoft.com/blog/celebrating-50-million-developers-the-journey-of-visual-studio-and-visual-studio-code) |    ✅    | ✅  |   ✅   |    ✅     |   ✅   |    ✅     |   ✅    | ⛔️ |
| IntelliJ / PyCharm             | [11.4M](https://www.jetbrains.com/lp/annualreport-2024/) |    ⛔️    | ⛔️ |   🌗   |    ⛔️     |   ⛔️   |    ⛔️     |   ⛔️    | ⛔️ |
| OpenVSX (Cursor, Windsurf etc) | [1M+ DAU](https://cursor.com/blog/series-d) |    ✅    | ✅  |   ✅   |    ✅     |   ✅   |    ✅     |   ✅    | ⛔️ |
| Emacs                          | [~1M](https://en.wikipedia.org/wiki/Emacs) |    ⛔️    | ⛔️ |   🌗   |    ⛔️     |   ⛔️   |    ⛔️     |   ⛔️    | ⛔️ |
| Vim                            | [~1/3 of CLI users](https://en.wikipedia.org/wiki/Vim_(text_editor)) |    ⛔️    | ⛔️ |   🌗   |    ⛔️     |   ⛔️   |    ⛔️     |   ⛔️    | ⛔️ |
| Sublime Text                   | [~1.5% mkt share](https://6sense.com/tech/ides-and-text-editors/sublime-text-market-share) |    ⛔️    | ⛔️ |   🌗   |    ⛔️     |   ⛔️   |    ⛔️     |   ⛔️    | ⛔️ |
| Zed                            | [100Ks/day](https://en.wikipedia.org/wiki/Zed_(text_editor)) |    ✅    | ⛔️  |   🌗   |    ✅     |   ✅   |    ✅     |   ✅    | ⛔️ |
| Neovim                         | [~180K](https://en.wikipedia.org/wiki/Neovim) |    🌗    | ✅  |   ⛔️   |    🌗     |   ✅   |    ✅     |   🌗    | ⛔️ |

User estimates link to their sources and are the platforms' own published figures, not our measurements.

## Next steps

Once installed, head to the [Quick Start](/docs/quick-start/) to run your first type check, or the [Configuration](/docs/configuration/) reference to tune Basilisk in `pyproject.toml`.
