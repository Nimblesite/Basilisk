--- LSP client configuration and lifecycle management.
---
--- Uses Neovim 0.11+ built-in LSP client (vim.lsp.config / vim.lsp.enable).
--- All 21 core LSP features are native — zero custom implementation needed.

local binary = require("basilisk.binary")

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

--- Configure and enable the basilisk LSP client.
---@param config BasiliskConfig
---@return boolean success
function M.start(config)
  local bin = binary.resolve(config.binary_path)
  if not bin then
    vim.notify(
      "[basilisk] binary not found. Install with: cargo install basilisk-cli",
      vim.log.levels.ERROR
    )
    return false
  end

  vim.lsp.config("basilisk", {
    cmd = { bin, "lsp" },
    filetypes = { "python" },
    root_markers = { "pyproject.toml", "setup.py", "setup.cfg", ".git" },
    settings = build_settings(config),
    init_options = {
      analysisMode = config.analysis_mode,
    },
  })

  vim.lsp.enable("basilisk")
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
    vim.notify(
      "[basilisk] max restarts reached (" .. MAX_RESTARTS .. "). Use :BasiliskRestart to force.",
      vim.log.levels.ERROR
    )
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
