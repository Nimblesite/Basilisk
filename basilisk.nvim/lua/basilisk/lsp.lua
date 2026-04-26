--- LSP client configuration and lifecycle management.
---
--- Uses Neovim 0.11+ built-in LSP client (vim.lsp.config / vim.lsp.enable).
--- All 21 core LSP features are native — zero custom implementation needed.

local binary = require("basilisk.binary")
local log = require("basilisk.log")

local M = {}

--- Maximum automatic restart attempts before giving up.
local MAX_RESTARTS = 3

--- Restart backoff delays in milliseconds: 1s, 2s, 4s.
local BACKOFF_MS = { 1000, 2000, 4000 }

--- Track restart state.
local restart_count = 0

--- Build the LSP settings table from the resolved config.
---@param config BasiliskConfig
---@return table
local function build_settings(config)
  return {
    basilisk = {
      enabled = config.enabled,
      python = config.python,
      analysisMode = config.analysis_mode,
      inlayHints = {
        parameterNames = config.inlay_hints.parameter_names,
        variableTypes = config.inlay_hints.variable_types,
      },
      ruff = {
        enabled = config.ruff.enabled,
        executablePath = config.ruff.executable_path,
      },
      debugger = {
        enabled = config.debugger.enabled,
        typeChecking = config.debugger.type_checking,
        debugpyPath = config.debugger.debugpy_path,
      },
      testExplorer = {
        enabled = config.test_explorer.enabled,
        framework = config.test_explorer.framework,
        pytestPath = config.test_explorer.pytest_path,
        args = config.test_explorer.args,
        autoDiscoverOnSave = config.test_explorer.auto_discover_on_save,
      },
      uv = {
        enabled = config.uv.enabled,
        executablePath = config.uv.executable_path,
        autoSync = config.uv.auto_sync,
        stubSuggestions = config.uv.stub_suggestions,
        dependencyDiagnostics = config.uv.dependency_diagnostics,
      },
    },
  }
end

--- Map LSP message types to Neovim notification levels.
---@param message_type integer?
---@return integer
local function lsp_message_level(message_type)
  local message_types = vim.lsp.protocol.MessageType
  if message_type == message_types.Error then
    return vim.log.levels.ERROR
  end
  if message_type == message_types.Warning then
    return vim.log.levels.WARN
  end
  return vim.log.levels.INFO
end

--- Display server showMessage notifications through the plugin logger.
---@param _err lsp.ResponseError?
---@param result lsp.ShowMessageParams?
local function handle_show_message(_err, result)
  if not result or type(result.message) ~= "string" or result.message == "" then
    return
  end
  local message = result.message:gsub("^Basilisk:%s*", "")
  local level = lsp_message_level(result.type)
  if level == vim.log.levels.ERROR then
    log.error("%s", message)
  elseif level == vim.log.levels.WARN then
    log.warn("%s", message)
  else
    log.info("%s", message)
  end
end

--- Route server logMessage notifications without using Neovim's headless error channel.
---@param _err lsp.ResponseError?
---@param result lsp.LogMessageParams?
local function handle_log_message(_err, result)
  if not result or type(result.message) ~= "string" or result.message == "" then
    return
  end
  local message = result.message:gsub("^Basilisk:%s*", "")
  local level = lsp_message_level(result.type)
  if level == vim.log.levels.ERROR then
    log.error("%s", message)
  elseif level == vim.log.levels.WARN then
    log.warn("%s", message)
  else
    log.debug("%s", message)
  end
end

--- Install Basilisk message handlers on already-running clients.
function M.install_handlers()
  for _, client in ipairs(vim.lsp.get_clients({ name = "basilisk" })) do
    client.handlers = client.handlers or {}
    client.handlers["window/logMessage"] = handle_log_message
    client.handlers["window/showMessage"] = handle_show_message
  end
end

--- Configure and enable the basilisk LSP client.
---@param config BasiliskConfig
---@return boolean success
function M.start(config)
  if config.binary_path and config.binary_path ~= "" and not binary.is_executable(config.binary_path) then
    log.error("binary not found: %s", config.binary_path)
    return false
  end

  local bin = binary.resolve(config.binary_path)
  if not bin then
    log.error("binary not found. Install with: cargo install basilisk-cli")
    return false
  end

  vim.lsp.config("basilisk", {
    cmd = { bin, "lsp" },
    filetypes = { "python" },
    root_markers = { "pyproject.toml", "setup.py", "setup.cfg", ".git" },
    settings = build_settings(config),
    handlers = {
      ["window/logMessage"] = handle_log_message,
      ["window/showMessage"] = handle_show_message,
    },
    init_options = {
      analysisMode = config.analysis_mode,
    },
  })

  vim.lsp.enable("basilisk")
  M.install_handlers()
  restart_count = 0
  return true
end

--- Restart the LSP server, respecting the backoff policy.
---@param config BasiliskConfig
---@param force? boolean Bypass the restart limit.
function M.restart(config, force)
  if force then
    restart_count = 0
  end

  if restart_count >= MAX_RESTARTS then
    log.error("max restarts reached (%d). Use :BasiliskRestart to force.", MAX_RESTARTS)
    return
  end

  local delay = BACKOFF_MS[restart_count + 1] or BACKOFF_MS[#BACKOFF_MS]
  restart_count = restart_count + 1

  vim.defer_fn(function()
    -- Stop all basilisk clients.
    for _, client in ipairs(vim.lsp.get_clients({ name = "basilisk" })) do
      client:stop()
    end
    -- Re-start after a tick so the stop completes.
    vim.defer_fn(function()
      M.start(config)
    end, 100)
  end, delay)
end

--- Reset the restart counter (called by :BasiliskRestart).
function M.reset_restart_count()
  restart_count = 0
end

--- Get the current restart count (for statusline).
---@return integer
function M.get_restart_count()
  return restart_count
end

return M
