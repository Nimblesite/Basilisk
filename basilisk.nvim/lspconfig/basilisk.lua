--- nvim-lspconfig server definition for Basilisk.
---
--- Implements [NVIM-DISTRIBUTION-SECONDARY-LSPCONFIG-PR] — the minimal LSP-only
--- config submitted to nvim-lspconfig for users who just want basic LSP.
---
--- This file is intended for submission to the nvim-lspconfig repository:
--- https://github.com/neovim/nvim-lspconfig
---
--- Users who install basilisk.nvim directly don't need this — the plugin
--- uses native vim.lsp.config/vim.lsp.enable. This config exists for users
--- who prefer nvim-lspconfig as their LSP management layer.

local util = require("lspconfig.util")

local bin_name = "basilisk"

local function find_binary()
  -- BASILISK_PATH env var.
  local env_path = vim.env.BASILISK_PATH
  if env_path and env_path ~= "" and vim.fn.executable(env_path) == 1 then
    return env_path
  end

  -- Well-known locations.
  local candidates = {
    vim.fn.expand("~/.cargo/bin/basilisk"),
    "/usr/local/bin/basilisk",
    "/opt/homebrew/bin/basilisk",
  }
  for _, candidate in ipairs(candidates) do
    if vim.fn.executable(candidate) == 1 then
      return candidate
    end
  end

  -- Fall back to PATH.
  return bin_name
end

return {
  default_config = {
    cmd = { find_binary(), "lsp" },
    filetypes = { "python" },
    root_dir = util.root_pattern("pyproject.toml", "setup.py", "setup.cfg", ".git"),
    single_file_support = true,
    settings = {
      basilisk = {
        analysisMode = "wholeModule",
        inlayHints = {
          parameterNames = true,
          variableTypes = true,
        },
        formatter = "ruff",
        uv = {
          enabled = true,
        },
      },
    },
    init_options = {
      analysisMode = "wholeModule",
    },
  },
  docs = {
    description = [[
https://github.com/Nimblesite/Basilisk

Basilisk is a strict-by-default Python type checker and comprehensive LSP
built in Rust. It provides type checking, inlay hints, code actions,
debugging, profiling, test exploration, and uv package manager integration.

Install with `cargo install basilisk-cli` or download from GitHub releases.

For the full-featured plugin (DAP, test explorer, profiling, keymaps), use
[basilisk.nvim](https://github.com/Nimblesite/Basilisk/tree/main/basilisk.nvim)
instead.
]],
    default_config = {
      root_dir = [[root_pattern("pyproject.toml", "setup.py", "setup.cfg", ".git")]],
    },
  },
}
