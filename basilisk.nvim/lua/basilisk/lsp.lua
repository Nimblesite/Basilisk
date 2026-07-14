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
      formatter = config.formatter,
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

--- Root-level configuration documents the server may edit on the user's behalf.
--- Matches the single discovery target in [CONFIGEDITOR-SOURCES]: the server
--- only ever edits `pyproject.toml` (`[tool.basilisk]`); a stray `basilisk.json`
--- is reported as shadowed, never read or edited.
local CONFIG_BASENAMES = { ["pyproject.toml"] = true }

--- Collect the file paths a WorkspaceEdit touches, across both encodings.
--- Handles `changes` (uri → edits) and `documentChanges` operations
--- (`TextDocumentEdit` and `Create`/`Rename`/`Delete` resource ops).
---@param edit table? lsp.WorkspaceEdit
---@return string[] paths Absolute filesystem paths, deduplicated.
local function collect_edit_paths(edit)
  local seen = {}
  local function add(uri)
    if type(uri) == "string" and uri ~= "" then
      seen[vim.uri_to_fname(uri)] = true
    end
  end
  if type(edit) ~= "table" then
    return {}
  end
  for uri in pairs(edit.changes or {}) do
    add(uri)
  end
  for _, change in ipairs(edit.documentChanges or {}) do
    add(change.uri or (change.textDocument and change.textDocument.uri))
  end
  return vim.tbl_keys(seen)
end

--- Persist a config-file buffer to disk after the server edited it.
--- Implements [CONFIGEDITOR-SOURCES]: a closed-source apply must become
--- "visible on disk" so the server's in-memory overlay can retire. Neovim's
--- default `workspace/applyEdit` only touches the buffer, so config documents
--- the user never opened stay unsaved without this explicit write.
---@param path string Absolute filesystem path of an edited document.
local function persist_config_document(path)
  if not CONFIG_BASENAMES[vim.fn.fnamemodify(path, ":t")] then
    return
  end
  local bufnr = vim.fn.bufnr(path)
  if bufnr < 0 or not vim.api.nvim_buf_is_loaded(bufnr) or not vim.bo[bufnr].modified then
    return
  end
  vim.api.nvim_buf_call(bufnr, function()
    vim.cmd("silent noautocmd keepalt write")
  end)
end

--- Apply a server-initiated `workspace/applyEdit`, then persist any edited
--- root configuration document to disk. Delegates the buffer edit to Neovim's
--- built-in handler so open buffers, undo, and position encoding stay correct.
---@param err lsp.ResponseError?
---@param params lsp.ApplyWorkspaceEditParams
---@param ctx lsp.HandlerContext
---@return lsp.ApplyWorkspaceEditResult
local function handle_apply_edit(err, params, ctx)
  local result = vim.lsp.handlers["workspace/applyEdit"](err, params, ctx)
  if result and result.applied then
    for _, path in ipairs(collect_edit_paths(params and params.edit)) do
      persist_config_document(path)
    end
  end
  return result
end

--- Install Basilisk message handlers on already-running clients.
function M.install_handlers()
  for _, client in ipairs(vim.lsp.get_clients({ name = "basilisk" })) do
    client.handlers = client.handlers or {}
    client.handlers["window/logMessage"] = handle_log_message
    client.handlers["window/showMessage"] = handle_show_message
    client.handlers["workspace/applyEdit"] = handle_apply_edit
  end
end

--- Configure and enable the basilisk LSP client.
--- Implements [NVIM-LSP-CLIENT-CONFIGURATION] — vim.lsp.config + vim.lsp.enable
--- with cmd/filetypes/root_markers/settings exactly as the spec documents.
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
      ["workspace/applyEdit"] = handle_apply_edit,
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
--- Implements [NVIM-LSP-CLIENT-CONFIGURATION-ERROR-RECOVERY] — auto-restart up to
--- MAX_RESTARTS with 1s/2s/4s exponential backoff; :BasiliskRestart forces a reset.
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
