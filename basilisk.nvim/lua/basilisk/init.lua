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
--- Implements [NVIM-LSP-CLIENT-CONFIGURATION-CUSTOM-COMMANDS] — installs
--- vim.lsp.commands handlers for server-advertised commands (the server is the
--- single source of truth; the plugin never registers a command it does not own).
local function register_lsp_commands()
  vim.lsp.commands["basilisk.organizeImports"] = function(cmd, ctx)
    local edit = cmd.edit or cmd.arguments and cmd.arguments[1]
    if edit then
      vim.lsp.util.apply_workspace_edit(edit, "utf-8")
    end
  end
end

--- Register notification handlers for basilisk/* server-push notifications.
local function register_notification_handlers()
  vim.lsp.handlers["basilisk/moduleChanged"] = function(_err, _result, _ctx, _config)
    local modules_ok, modules_panel = pcall(require, "basilisk.modules")
    if modules_ok then
      modules_panel.refresh()
    end
    local health_ok, health_panel = pcall(require, "basilisk.type_health")
    if health_ok then
      health_panel.refresh()
    end
  end

  -- Profiler progress: update statusline while a session is running.
  vim.lsp.handlers["basilisk/profiler/progress"] = function(_err, result, _ctx, _config)
    if not result then
      return
    end
    local statusline = require("basilisk.statusline")
    local samples = result.totalSamples or 0
    local elapsed = result.elapsedSeconds or 0
    local pid = result.pid or 0
    log.info("profiling PID %d: %ds, %d samples", pid, elapsed, samples)
    statusline.set_profiler_status(result)
  end

  -- Memory timeline: periodic snapshot data during auto-snapshot mode.
  vim.lsp.handlers["basilisk/memory/timeline"] = function(_err, result, _ctx, _config)
    if not result then
      return
    end
    log.info(
      "memory timeline: current=%d peak=%d",
      result.currentMemory or 0,
      result.peakMemory or 0
    )
  end
end

--- Set up default keymaps for activity panels.
---@param cfg BasiliskConfig
local function register_keymaps(cfg)
  if not cfg.keymaps.enabled then
    return
  end
  local prefix = cfg.keymaps.prefix or "<leader>b"
  vim.keymap.set("n", prefix .. "m", "<cmd>BasiliskModules<CR>", { desc = "Toggle Basilisk Module Explorer" })
  vim.keymap.set("n", prefix .. "h", "<cmd>BasiliskHealth<CR>", { desc = "Toggle Basilisk Type Health" })
  vim.keymap.set("n", prefix .. "i", "<cmd>BasiliskInfo<CR>", { desc = "Show Basilisk Server Info" })
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

  -- Register module change notification handler.
  register_notification_handlers()

  -- Start the LSP client.
  local started = lsp.start(M.config)

  -- Check for updates asynchronously after successful start.
  if started then
    local bin = require("basilisk.binary")
    local bin_path = bin.resolve(M.config.binary_path)
    if bin_path then
      bin.check_for_updates(bin_path)
    end
  end

  -- Register user commands.
  commands.register(M.config)

  -- Register default keymaps for activity panels.
  register_keymaps(M.config)

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
