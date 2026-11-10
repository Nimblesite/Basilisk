---
layout: layouts/docs.njk
title: "Best Python VS Code Extension? Install Basilisk"
description: "Looking for the best Python VS Code extension? See when Basilisk fits, then install its bundled type checker, LSP, debugger, profiler, and refactoring tools."
keywords: best python vscode extension, python extension for vscode, basilisk, vs code, cursor, windsurf, open vsx, python language server, install, vsix
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: VS Code & Cursor
  parent: Installation
  order: 1
---

# VS Code, Cursor & Windsurf

Install the **Basilisk** extension from your editor's marketplace:

1. Open your editor
2. Go to Extensions (`Ctrl+Shift+X` / `Cmd+Shift+X`)
3. Search for **Basilisk**
4. Click **Install**

The extension is published to the **[VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=Nimblesite.basilisk)** and to **[Open VSX](https://open-vsx.org/extension/Nimblesite/basilisk)**, so it installs in **VS Code**, **Cursor**, **Windsurf**, and other VS Code-compatible editors.

Open a Python file and Basilisk activates automatically — diagnostics, completions, hover, go-to-definition, rename, refactoring, formatting, debugging (F5), and profiling.

![Basilisk in VS Code — PEP-conformant type errors shown inline with red squiggles and listed in the Problems panel](/assets/images/vscode-diagnostics.png)

*PEP-conformant diagnostics the moment you open a file — no configuration.*

## Is Basilisk the best Python VS Code extension for you?

No Python extension is best for every project. Basilisk is designed for developers who want one open-source extension for typing-spec-conformant checking, completions, navigation, refactoring, formatting, debugging, and profiling — with the same language server available outside VS Code. If your project depends on mature mypy framework plugins or you prefer Pylance's established VS Code-only workflow, review the [Python type checker comparison](/docs/comparison/) before switching.

## The binary is bundled — no separate install

**The extension ships the matching Basilisk binary for your platform inside the VSIX.** A default install needs no extra setup: no `cargo install`, no PATH configuration, no manual download.

| OS | Architecture |
|----|-------------|
| macOS | Apple Silicon (aarch64) |
| Linux | x86_64 |
| Linux | aarch64 |
| Windows | x86_64 |
| Windows | arm64 |

## How the extension finds the binary

The extension resolves the binary in this order:

1. **Explicit component path** — `basilisk.binaries.basilisk` or `basilisk.executablePath`
2. **Explicit binary directory** — `basilisk.binaries.path`
3. **Bundled VSIX binary** — `bin/<platform>/basilisk` (the default)
4. **External install** — Cargo, Homebrew, Scoop, or PATH, if the version matches

Homebrew and Scoop act as external override or repair sources. A default install runs the binary bundled inside the VSIX. Use `basilisk.executablePath`, `basilisk.binaries.basilisk`, or `basilisk.binaries.path` only when you intentionally want to override the bundled binary — for example, to run a locally built development binary.

## Next steps

- [Quick Start](/docs/quick-start/) — your first type check
- [Debugging](/docs/debugging/) — press F5 to debug
- [Configuration](/docs/configuration/) — `pyproject.toml` reference
