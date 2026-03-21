# basilisk.nvim

First-class Neovim plugin for Basilisk — zero-config Python type checking, debugging, profiling, and test exploration.

## Role in Basilisk

This is the **Neovim editor integration**. It connects Neovim's built-in LSP client to the Basilisk language server, providing the same feature set as the VS Code extension: real-time diagnostics, hover, go-to-definition, code actions, inlay hints, integrated debugging, and profiling.

## Features

- **Zero-config setup** — detects the `basilisk` binary and connects automatically
- **Real-time diagnostics** — errors appear inline as you type
- **Go-to-definition, hover, find references** — full LSP navigation
- **Code actions & refactoring** — extract, rename, move, inline
- **Inlay hints** — parameter names and inferred types
- **Integrated debugging** — nvim-dap compatible, F5 to debug
- **Test explorer** — discover and run pytest tests from the editor
- **Python profiling** — py-spy heatmaps directly in the editor
- **Memory leak tracking** — detect leaks during development
- **uv integration** — `uv sync` and `uv add` commands
- **Status line** — LSP status in your status line
- **Health checks** — `:checkhealth basilisk` for diagnostics

## Requirements

- Neovim 0.10+
- The `basilisk` binary in PATH (or configured explicitly)

## Quick start

```lua
require("basilisk").setup()
```

See [doc/basilisk.txt](doc/basilisk.txt) for full documentation.

## License

MIT or Apache-2.0, at your option.
