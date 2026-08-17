--- Auto-loaded entry point for basilisk.nvim.
--- Guards against double-loading and provides the setup trigger.

if vim.g.loaded_basilisk then
  return
end
vim.g.loaded_basilisk = true

-- Defer actual setup to require('basilisk').setup() so users control
-- when configuration is applied. This file only sets the guard.
