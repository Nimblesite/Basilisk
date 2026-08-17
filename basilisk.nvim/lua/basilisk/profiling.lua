--- Profiling commands for Basilisk.
---
--- Sends LSP profiler commands and displays results in floating windows,
--- quickfix lists, and heat map extmarks.

local log = require("basilisk.log")
local ui = require("basilisk.ui")

local M = {}

--- Namespace for profiling extmarks (heat map).
local ns = vim.api.nvim_create_namespace("basilisk-profiling")

--- Active profiling session ID.
---@type string?
local session_id = nil

--- Start profiling.
---@param pid? integer Optional process ID to profile.
function M.start(pid)
  local client = ui.get_client()
  if not client then
    log.warn("no active LSP client")
    return
  end

  local args = {}
  if pid then
    args = { { pid = pid } }
  end

  client:request("workspace/executeCommand", {
    command = "basilisk.profiler.start",
    arguments = args,
  }, function(err, result)
    if err then
      log.error("profiler start failed: %s", err.message or tostring(err))
      return
    end
    if result and result.sessionId then
      session_id = result.sessionId
    end
    log.info("profiling started")
  end, 0)
end

--- Stop profiling and display results.
function M.stop()
  local client = ui.get_client()
  if not client then
    log.warn("no active LSP client")
    return
  end

  local args = {}
  if session_id then
    args = { { sessionId = session_id } }
  end

  client:request("workspace/executeCommand", {
    command = "basilisk.profiler.stop",
    arguments = args,
  }, function(err, result)
    if err then
      log.error("profiler stop failed: %s", err.message or tostring(err))
      return
    end
    session_id = nil
    vim.schedule(function()
      M.display_results(result)
    end)
  end, 0)
end

--- Take a snapshot without stopping.
function M.snapshot()
  local client = ui.get_client()
  if not client then
    log.warn("no active LSP client")
    return
  end

  local args = {}
  if session_id then
    args = { { sessionId = session_id } }
  end

  client:request("workspace/executeCommand", {
    command = "basilisk.profiler.snapshot",
    arguments = args,
  }, function(err, result)
    if err then
      log.error("profiler snapshot failed: %s", err.message or tostring(err))
      return
    end
    vim.schedule(function()
      M.display_results(result)
    end)
  end, 0)
end

--- Display profiling results in a floating window and quickfix list.
--- Implements [NVIM-USER-COMMANDS-PROFILING-UI] — hot-function list in a float +
--- quickfix list, with heat-map extmarks (apply_heat_map) and speedscope export
--- (export_flamegraph) for the flamegraph view.
---@param result? table Profiling results from the LSP server.
function M.display_results(result)
  if not result then
    ui.open_float("Profiling Results", { "No profiling data available." }, "basilisk-profiling")
    return
  end

  local lines = { "Hot Functions:", "" }
  local qf_items = {}

  local hot_functions = result.hotFunctions or {}
  for i, func in ipairs(hot_functions) do
    local line = string.format(
      "%3d. %6.1f%%  %s  (%s:%d)",
      i,
      func.percentage or 0,
      func.name or "?",
      func.file or "?",
      func.line or 0
    )
    lines[#lines + 1] = line
    qf_items[#qf_items + 1] = {
      filename = func.file,
      lnum = func.line or 0,
      text = string.format("%.1f%% — %s", func.percentage or 0, func.name or "?"),
    }
  end

  if #hot_functions == 0 then
    lines[#lines + 1] = "  (no hot functions recorded)"
  end

  -- Show floating window.
  ui.open_float("Profiling Results", lines, "basilisk-profiling")

  -- Populate quickfix list.
  if #qf_items > 0 then
    vim.fn.setqflist(qf_items, "r")
    log.info("profiling results added to quickfix list (:copen)")
  end

  -- Apply heat map extmarks.
  M.apply_heat_map(hot_functions)
end

--- Apply heat map extmarks on hot lines.
---@param hot_functions table[]
function M.apply_heat_map(hot_functions)
  -- Clear previous heat map.
  for _, buf in ipairs(vim.api.nvim_list_bufs()) do
    if vim.api.nvim_buf_is_loaded(buf) then
      vim.api.nvim_buf_clear_namespace(buf, ns, 0, -1)
    end
  end

  for _, func in ipairs(hot_functions or {}) do
    local file = func.file
    local line = (func.line or 1) - 1
    local pct = func.percentage or 0
    if file then
      -- Find buffer for this file.
      for _, buf in ipairs(vim.api.nvim_list_bufs()) do
        local name = vim.api.nvim_buf_get_name(buf)
        if name == file and vim.api.nvim_buf_is_loaded(buf) then
          local hl = pct > 50 and "DiagnosticError"
            or pct > 20 and "DiagnosticWarn"
            or "DiagnosticHint"
          pcall(vim.api.nvim_buf_set_extmark, buf, ns, line, 0, {
            virt_text = { { string.format(" %.1f%%", pct), hl } },
            virt_text_pos = "eol",
          })
        end
      end
    end
  end
end

--- Open the flamegraph SVG exported by the LSP server in the browser.
--- Implements [PROFILE-VIEWER-DELIVERY]: speedscope.app can NEVER fetch a
--- `file://` profileURL (an https page may not read local files), so we open
--- the local self-contained SVG instead and log the speedscope JSON path for
--- manual import at https://www.speedscope.app.
---@param result? table Profiling results from the LSP `profiler.stop` response.
function M.export_flamegraph(result)
  if not result or not result.flamegraphPath then
    local reason = result and result.exportError or "no flamegraph available"
    log.warn("flamegraph export unavailable: %s", reason)
    return
  end
  if vim.fn.filereadable(result.flamegraphPath) == 0 then
    log.error("flamegraph file missing: %s", result.flamegraphPath)
    return
  end

  vim.ui.open("file://" .. result.flamegraphPath)
  log.info("flamegraph opened: %s", result.flamegraphPath)
  if result.outputFile then
    log.info(
      "speedscope JSON: %s (import manually at https://www.speedscope.app)",
      result.outputFile
    )
  end
end

return M
