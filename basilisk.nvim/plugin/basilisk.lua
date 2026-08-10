--- Auto-loaded entry point for basilisk.nvim.
---
--- Implements [WITHDRAWAL-SURFACES]. The plugin is a notice: loading it says so
--- once per session and does nothing else. Nothing is registered, so removing
--- the plugin from a config is the only remaining action.

if vim.g.loaded_basilisk then
  return
end
vim.g.loaded_basilisk = true

vim.schedule(function()
  require("basilisk").announce()
end)
