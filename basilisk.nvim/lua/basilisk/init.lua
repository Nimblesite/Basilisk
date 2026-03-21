--- Basilisk — a strict-by-default Python type checker for Neovim.
---
--- Entry point: require('basilisk').setup({})
--- Zero-config works out of the box.

local config_mod = require("basilisk.config")
local lsp = require("basilisk.lsp")
local commands = require("basilisk.commands")
local log = require("basilisk.log")

local M = {}

--- Resolved configuration (populated after setup).
---@type BasiliskConfig?
M.config = nil

--- Whether setup() has been called.
local did_setup = false

--- Register LSP command handlers for custom commands.
local function register_lsp_commands()
  vim.lsp.commands["basilisk.organizeImports"] = function(cmd, ctx)
    local edit = cmd.edit or cmd.arguments and cmd.arguments[1]
    if edit then
      vim.lsp.util.apply_workspace_edit(edit, "utf-8")
    end
  end
end

--- Set up Basilisk with the given options.
---@param opts? table User configuration (merged with defaults).
function M.setup(opts)
  if did_setup then
    return
  end
  did_setup = true

  M.config = config_mod.resolve(opts)

  -- Configure logging.
  log.set_level(M.config.log_level)
  log.info("setup started")

  -- Register custom LSP command handlers.
  register_lsp_commands()

  -- Start the LSP client.
  lsp.start(M.config)

  -- Register user commands.
  commands.register(M.config)

  -- Register DAP adapter if nvim-dap is available.
  local dap_ok, dap_mod = pcall(require, "basilisk.dap")
  if dap_ok then
    dap_mod.setup(M.config)
  end

  -- Set up tab tracking for openFilesOnly mode.
  local tab_tracking = require("basilisk.tab_tracking")
  tab_tracking.setup(M.config)

  log.info("setup complete")
end

return M
