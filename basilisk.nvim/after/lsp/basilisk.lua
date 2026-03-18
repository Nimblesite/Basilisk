--- Neovim 0.11+ native LSP config for basilisk.
---
--- This file is auto-discovered by Neovim's built-in LSP framework.
--- It provides a fallback for users who don't call require('basilisk').setup()
--- but still want basic LSP functionality.

local binary = require("basilisk.binary")

local bin = binary.resolve()
if not bin then
  return
end

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
