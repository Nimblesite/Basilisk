---
layout: layouts/docs.njk
title: "Basilisk for Neovim — Install & Update basilisk.nvim"
description: "Install the Basilisk Python language server in Neovim with basilisk.nvim. The binary downloads automatically; update it in-editor with :BasiliskUpdate. Diagnostics, completions, debugging, tests, and profiling via the built-in LSP client."
keywords: basilisk, neovim, nvim, basilisk.nvim, python, language server, lsp, install, update, lazy.nvim, packer, vim-plug, nvim-dap
date: 2026-07-11
dateModified: 2026-07-11
author: The Basilisk Project
eleventyNavigation:
  key: Neovim
  parent: Installation
  order: 3
---

# Basilisk for Neovim

[`basilisk.nvim`](https://github.com/Nimblesite/basilisk.nvim) connects Neovim's built-in LSP client (0.11+, via `vim.lsp.config` / `vim.lsp.enable`) to the Basilisk language server. One plugin covers the whole workflow: diagnostics, hover, completions, go-to-definition, rename, code actions, formatting, inlay hints, debugging (via nvim-dap), a test explorer, and profiling.

Two parts get installed — the **plugin** (via your plugin manager) and the **`basilisk` binary** (downloaded automatically; you normally never install it yourself).

## 1. Install the plugin

**lazy.nvim**

```lua
{
  "Nimblesite/basilisk.nvim",
  ft = "python",
  dependencies = { "mfussenegger/nvim-dap" }, -- optional, for debugging
  opts = {},
}
```

**packer.nvim**

```lua
use {
  "Nimblesite/basilisk.nvim",
  ft = "python",
  config = function()
    require("basilisk").setup({})
  end,
}
```

**vim-plug**

```vim
Plug 'Nimblesite/basilisk.nvim'
```

then, after `plug#end()`:

```lua
lua require("basilisk").setup({})
```

**vim.pack (built-in, Neovim 0.12+)**

```lua
vim.pack.add({
  { src = "https://github.com/Nimblesite/basilisk.nvim",
    version = vim.version.range("*") }, -- latest stable tag
})
require("basilisk").setup({})
```

## 2. The binary comes with the plugin

**You do not install the Basilisk binary separately.** Open a Python file: if no `basilisk` binary is found, the plugin downloads the latest release for your platform from the [GitHub releases](https://github.com/Nimblesite/Basilisk/releases), caches it in Neovim's data directory, and starts the language server. You can also trigger this explicitly with `:BasiliskInstall`.

Already have Basilisk installed — via [Homebrew, Scoop, or cargo](/docs/install-cli/), or on your `PATH`? The plugin finds and uses that install instead.

Verify everything with `:checkhealth basilisk`.

## Updating

The plugin and the binary update separately:

- **Plugin** — like any other plugin: `:Lazy update`, `:PackerSync`, or `:PlugUpdate`.
- **Binary** — when a newer release exists, a startup notice tells you. Run **`:BasiliskUpdate`**: it asks for confirmation, downloads the new version, and restarts the LSP in place. If your binary is owned by a package manager, Basilisk never overwrites it — the notice names the right command instead (`brew upgrade basilisk`, `scoop update basilisk`, or `cargo install --git https://github.com/Nimblesite/Basilisk basilisk-cli`).

## Configure Basilisk settings

Zero-config works out of the box. To adjust behavior, pass options to `setup()`:

```lua
require("basilisk").setup({
  analysis_mode = "wholeModule", -- "openFilesOnly" | "wholeModule" | "crossModule"
  inlay_hints = {
    parameter_names = true,
    variable_types = true,
  },
  formatter = "ruff", -- embedded in the basilisk binary; or "none"
})
```

The full option list (debugger, test explorer, uv integration, keymaps, statusline) is in the plugin's help file: `:h basilisk-configuration`.

## Debugging, tests, and profiling

- **Debugging** — with [nvim-dap](https://github.com/mfussenegger/nvim-dap) installed, `:BasiliskDebugFile` (or your DAP keymaps) debugs the current file through `debugpy`. See [Debugging](/docs/debugging/).
- **Test explorer** — `:BasiliskTestToggle` opens the test panel; `:BasiliskTestRun` runs tests.
- **Profiling** — `:BasiliskProfile` / `:BasiliskProfileStop` drive the built-in profiler. See the [Profiler](/docs/profiler/) guide.

All commands are listed under `:h basilisk-commands`.

## Advanced: override the binary

For development builds or a specific system install, point the plugin at an explicit path:

```lua
require("basilisk").setup({
  binary_path = "/absolute/path/to/basilisk",
})
```

…or set the `BASILISK_PATH` environment variable. The setting takes precedence; with neither set, the plugin uses the resolution cascade above.

## Next steps

- [Quick Start](/docs/quick-start/) — your first type check
- [Configuration](/docs/configuration/) — `pyproject.toml` reference
- [Refactoring](/docs/refactoring/) — extract, inline, move, and more
