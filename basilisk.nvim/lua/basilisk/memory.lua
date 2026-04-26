--- Memory tracking commands for Basilisk.
---
--- Sends LSP memory commands and displays leak reports and retention
--- paths in floating windows.

local log = require("basilisk.log")
local ui = require("basilisk.ui")

local M = {}

--- Active memory tracking session ID.
---@type string?
local session_id = nil

--- Common types for :BasiliskMemRefs completion.
local COMMON_TYPES = {
  "DataFrame",
  "Series",
  "Tensor",
  "ndarray",
  "dict",
  "list",
  "set",
  "tuple",
  "str",
  "bytes",
  "int",
  "float",
}

--- Start memory leak tracking.
function M.start()
  local client = ui.get_client()
  if not client then
    log.warn("no active LSP client")
    return
  end

  client:request("workspace/executeCommand", {
    command = "basilisk.memory.start",
    arguments = {},
  }, function(err, result)
    if err then
      log.error("memory start failed: %s", err.message or tostring(err))
      return
    end
    if result and result.sessionId then
      session_id = result.sessionId
    end
    log.info("memory tracking started")
  end, 0)
end

--- Stop memory tracking and display leak report.
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
    command = "basilisk.memory.diff",
    arguments = args,
  }, function(err, result)
    if err then
      log.error("memory stop failed: %s", err.message or tostring(err))
      return
    end
    session_id = nil
    vim.schedule(function()
      M.display_leak_report(result)
    end)
  end, 0)
end

--- Query retention paths for a type.
---@param type_name string
function M.refs(type_name)
  local client = ui.get_client()
  if not client then
    log.warn("no active LSP client")
    return
  end

  client:request("workspace/executeCommand", {
    command = "basilisk.memory.references",
    arguments = { { typeName = type_name } },
  }, function(err, result)
    if err then
      log.error("memory refs failed: %s", err.message or tostring(err))
      return
    end
    vim.schedule(function()
      M.display_retention_paths(type_name, result)
    end)
  end, 0)
end

--- Display a leak report in a floating window.
---@param result? table Leak report from the LSP server.
function M.display_leak_report(result)
  if not result then
    ui.open_float("Memory Leak Report", { "No leak data available." }, "basilisk-memory")
    return
  end

  local lines = { "Memory Leak Report", "" }
  local leaks = result.leaks or {}

  for _, leak in ipairs(leaks) do
    lines[#lines + 1] = string.format(
      "  %s: %d objects, %s",
      leak.typeName or "?",
      leak.count or 0,
      leak.totalSize or "?"
    )
    if leak.location then
      lines[#lines + 1] = string.format("    at %s:%d", leak.location.file or "?", leak.location.line or 0)
    end
  end

  if #leaks == 0 then
    lines[#lines + 1] = "  No leaks detected."
  end

  ui.open_float("Memory Leak Report", lines, "basilisk-memory")
end

--- Display retention paths in a floating window.
---@param type_name string
---@param result? table Retention paths from the LSP server.
function M.display_retention_paths(type_name, result)
  if not result then
    ui.open_float("Retention Paths: " .. type_name, { "No retention data available." }, "basilisk-memory")
    return
  end

  local lines = { "Retention Paths for: " .. type_name, "" }
  local paths = result.retentionPaths or {}

  for i, path in ipairs(paths) do
    local confidence = path.confidence or 0
    lines[#lines + 1] = string.format("  Path %d (confidence: %.0f%%):", i, confidence * 100)
    for _, step in ipairs(path.steps or {}) do
      lines[#lines + 1] = string.format("    -> %s (%s)", step.name or "?", step.kind or "?")
    end
    lines[#lines + 1] = ""
  end

  if #paths == 0 then
    lines[#lines + 1] = "  No retention paths found."
  end

  ui.open_float("Retention Paths: " .. type_name, lines, "basilisk-memory")
end

--- Completion function for :BasiliskMemRefs.
---@param lead string
---@return string[]
function M.complete_refs(lead)
  local matches = {}
  for _, t in ipairs(COMMON_TYPES) do
    if t:lower():find(lead:lower(), 1, true) then
      matches[#matches + 1] = t
    end
  end
  return matches
end

return M
