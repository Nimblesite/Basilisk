--- User commands for Basilisk.
---
--- All profiling/memory/test LSP commands (defined in LSP-SPEC.md)
--- surface as :Basilisk* user commands.

local M = {}

--- Get the first active basilisk LSP client, or nil.
---@return vim.lsp.Client?
local function get_client()
  local clients = vim.lsp.get_clients({ name = "basilisk" })
  return clients[1]
end

--- Send an LSP executeCommand request.
---@param command string
---@param args? table
local function execute_command(command, args)
  local client = get_client()
  if not client then
    vim.notify("[basilisk] no active LSP client", vim.log.levels.WARN)
    return
  end
  client:request("workspace/executeCommand", {
    command = command,
    arguments = args or {},
  }, nil, 0)
end

--- Register all :Basilisk* commands.
---@param config BasiliskConfig
function M.register(config)
  local lsp_mod = require("basilisk.lsp")

  vim.api.nvim_create_user_command("BasiliskRestart", function()
    lsp_mod.reset_restart_count()
    lsp_mod.restart(config, true)
    vim.notify("[basilisk] restarting server...", vim.log.levels.INFO)
  end, { desc = "Restart the Basilisk LSP server" })

  vim.api.nvim_create_user_command("BasiliskInfo", function()
    local client = get_client()
    if not client then
      vim.notify("[basilisk] no active LSP client", vim.log.levels.WARN)
      return
    end
    vim.notify(
      string.format(
        "[basilisk] client id=%d, root=%s, restarts=%d",
        client.id,
        client.root_dir or "nil",
        lsp_mod.get_restart_count()
      ),
      vim.log.levels.INFO
    )
  end, { desc = "Show Basilisk LSP server info" })

  vim.api.nvim_create_user_command("BasiliskOrganizeImports", function()
    local uri = vim.uri_from_bufnr(vim.api.nvim_get_current_buf())
    execute_command("basilisk.organizeImports", { uri })
  end, { desc = "Organize imports via Basilisk" })

  vim.api.nvim_create_user_command("BasiliskProfile", function(opts)
    local args = {}
    if opts.args ~= "" then
      args = { { pid = tonumber(opts.args) } }
    end
    execute_command("basilisk/profiler/start", args)
  end, { nargs = "?", desc = "Start profiling" })

  vim.api.nvim_create_user_command("BasiliskProfileStop", function()
    execute_command("basilisk/profiler/stop")
  end, { desc = "Stop profiling" })

  vim.api.nvim_create_user_command("BasiliskProfileSnapshot", function()
    execute_command("basilisk/profiler/snapshot")
  end, { desc = "Take profiling snapshot" })

  vim.api.nvim_create_user_command("BasiliskMemLeak", function()
    execute_command("basilisk/memory/start")
  end, { desc = "Start memory leak tracking" })

  vim.api.nvim_create_user_command("BasiliskMemStop", function()
    execute_command("basilisk/memory/stop")
  end, { desc = "Stop memory tracking" })

  vim.api.nvim_create_user_command("BasiliskMemRefs", function(opts)
    execute_command("basilisk/memory/refs", { { type = opts.args } })
  end, { nargs = 1, desc = "Show memory references for a type" })

  vim.api.nvim_create_user_command("BasiliskDebugFile", function()
    execute_command("basilisk/startDebugSession", {
      { python = config.python },
    })
  end, { desc = "Start debugging current file" })
end

return M
