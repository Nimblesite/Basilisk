--- Basilisk — a strict-by-default Python type checker for Neovim.
---
--- Entry point: require('basilisk').setup({})
--- Zero-config works out of the box.

local config_mod = require("basilisk.config")
local lsp = require("basilisk.lsp")
local commands = require("basilisk.commands")

local M = {}

--- Resolved configuration (populated after setup).
---@type BasiliskConfig?
M.config = nil

--- Whether setup() has been called.
local did_setup = false

--- Set up Basilisk with the given options.
---@param opts? table User configuration (merged with defaults).
function M.setup(opts)
  if did_setup then
    return
  end
  did_setup = true

  M.config = config_mod.resolve(opts)

  -- Start the LSP client.
  lsp.start(M.config)

  -- Register user commands.
  commands.register(M.config)
end

return M
