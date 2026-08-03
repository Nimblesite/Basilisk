--- Neovim 0.11+ native LSP config for basilisk.
---
--- This file is auto-discovered by Neovim's built-in LSP framework.
--- It provides a fallback for users who don't call require('basilisk').setup()
--- but still want basic LSP functionality.

local binary = require("basilisk.binary")

-- Neovim's LSP loader requires this file to evaluate to a table — a bare
-- `return` surfaces as "after/lsp/basilisk.lua: not a table" (issue #370).
-- When nothing resolves, degrade to the bare command name exactly as the
-- nvim-lspconfig definition does: the client then fails to spawn with a
-- readable "command not found" instead of a Lua error, and starts working the
-- moment a basilisk lands on PATH — no reload needed.
local bin = binary.resolve() or "basilisk"

return {
  cmd = { bin, "lsp" },
  filetypes = { "python" },
  root_markers = { "pyproject.toml", "setup.py", "setup.cfg", ".git" },
  settings = {
    basilisk = {
      analysisMode = "wholeModule",
    },
  },
  init_options = {
    analysisMode = "wholeModule",
  },
}
