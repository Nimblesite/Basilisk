<p align="center"><strong>English</strong> · <a href="README.zh.md">简体中文</a></p>

# basilisk.nvim

First-class Neovim plugin for Basilisk — zero-config Python type checking, debugging, profiling, and test exploration.

Basilisk is the only Python type checker scoring 100% on the [official `python/typing` conformance suite](https://github.com/python/typing/blob/main/conformance/results/results.html) — and the fastest we've measured. A complete, open-source Python dev environment in Rust: type checker, language server, debugger, profiler, plus VS Code, Cursor, Zed & Neovim extensions. Strict by default.

<p align="center">
  <img src="https://raw.githubusercontent.com/Nimblesite/Basilisk/main/website/src/assets/images/screenshot.png" alt="Basilisk in action — type checking, diagnostics, and refactoring in the editor" width="900">
</p>

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

- Neovim 0.11+ (the plugin uses the built-in `vim.lsp.config` / `vim.lsp.enable` API)
- `curl` (used once, to download the `basilisk` binary — see below)

## Install

Two parts get installed: the **plugin** (this repo, via your plugin manager) and the **`basilisk` binary** (downloaded automatically — you normally never install it yourself).

### 1. Install the plugin

<details open>
<summary><strong>lazy.nvim</strong></summary>

```lua
{
  "Nimblesite/basilisk.nvim",
  ft = "python",
  dependencies = { "mfussenegger/nvim-dap" }, -- optional, for debugging
  opts = {},
}
```
</details>

<details>
<summary><strong>packer.nvim</strong></summary>

```lua
use {
  "Nimblesite/basilisk.nvim",
  ft = "python",
  config = function()
    require("basilisk").setup({})
  end,
}
```
</details>

<details>
<summary><strong>vim-plug</strong></summary>

```vim
Plug 'Nimblesite/basilisk.nvim'
```

then somewhere after `plug#end()`:

```lua
lua require("basilisk").setup({})
```
</details>

<details>
<summary><strong>vim.pack (built-in, Neovim 0.12+)</strong></summary>

```lua
vim.pack.add({
  { src = "https://github.com/Nimblesite/basilisk.nvim",
    version = vim.version.range("*") }, -- latest stable tag; or pin "v0.33.0"
})
require("basilisk").setup({})
```
</details>

### 2. The binary installs itself

Open any Python file. If no `basilisk` binary is found, the plugin downloads the latest [GitHub release](https://github.com/Nimblesite/Basilisk/releases) for your platform into Neovim's data directory and starts the LSP — no PATH setup, no manual step. You can also trigger it explicitly with `:BasiliskInstall`.

Prefer a package manager? The plugin picks up existing installs automatically:

```sh
# macOS (Apple Silicon) / Linux
brew tap Nimblesite/tap && brew install basilisk

# Windows
scoop bucket add nimblesite https://github.com/Nimblesite/scoop-bucket
scoop install basilisk

# anywhere with a Rust toolchain
cargo install basilisk-cli
```

That's it — diagnostics, hover, completions, formatting, debugging, tests, and profiling all run through this one plugin. Verify with `:checkhealth basilisk`.

## Updating

- **Plugin**: update like any other plugin — `:Lazy update` (lazy.nvim), `:PackerSync` (packer), `:PlugUpdate` (vim-plug).
- **Binary**: when a new release is out, the plugin notifies you on startup. Run **`:BasiliskUpdate`** — it confirms, downloads the new version, and restarts the LSP in place. Installs owned by a package manager are never overwritten; the notice tells you to run `brew upgrade basilisk` / `scoop update basilisk` / `cargo install basilisk-cli` instead.

## Configuration

Zero-config works out of the box:

```lua
require("basilisk").setup()
```

All options (analysis mode, inlay hints, formatter, debugger, test explorer, uv, keymaps…) are documented in [doc/basilisk.txt](doc/basilisk.txt) — `:h basilisk-configuration`.

## License

MIT.
