--- Memory tracking commands for Basilisk.
---
--- Sends LSP memory commands and displays leak reports and retention
--- paths in floating windows.

local log = require("basilisk.log")

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

--- Open a floating window with the given lines.
---@param title string
---@param lines string[]
---@return integer buf, integer win
local function open_float(title, lines)
  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].modifiable = false
  vim.bo[buf].bufhidden = "wipe"
  vim.bo[buf].filetype = "basilisk-memory"

  local width = 80
  local height = math.min(#lines, 30)
  for _, line in ipairs(lines) do
    width = math.max(width, #line + 2)
  end
  width = math.min(width, math.floor(vim.o.columns * 0.8))

  local win = vim.api.nvim_open_win(buf, true, {
    relative = "editor",
    width = width,
    height = height,
    col = math.floor((vim.o.columns - width) / 2),
    row = math.floor((vim.o.lines - height) / 2),
    style = "minimal",
    border = "rounded",
    title = " " .. title .. " ",
    title_pos = "center",
  })

  vim.keymap.set("n", "q", function()
    vim.api.nvim_win_close(win, true)
  end, { buffer = buf })

  return buf, win
end

--- Get the first active basilisk LSP client, or nil.
---@return vim.lsp.Client?
local function get_client()
  local clients = vim.lsp.get_clients({ name = "basilisk" })
  return clients[1]
end

--- Start memory leak tracking.
function M.start()
  local client = get_client()
  if not client then
    log.warn("no active LSP client")
    return
  end

  client:request("workspace/executeCommand", {
    command = "basilisk/memory/start",
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
  local client = get_client()
  if not client then
    log.warn("no active LSP client")
    return
  end

  local args = {}
  if session_id then
    args = { { sessionId = session_id } }
  end

  client:request("workspace/executeCommand", {
    command = "basilisk/memory/stop",
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
  local client = get_client()
  if not client then
    log.warn("no active LSP client")
    return
  end

  client:request("workspace/executeCommand", {
    command = "basilisk/memory/refs",
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
    open_float("Memory Leak Report", { "No leak data available." })
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

  open_float("Memory Leak Report", lines)
end

--- Display retention paths in a floating window.
---@param type_name string
---@param result? table Retention paths from the LSP server.
function M.display_retention_paths(type_name, result)
  if not result then
    open_float("Retention Paths: " .. type_name, { "No retention data available." })
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

  open_float("Retention Paths: " .. type_name, lines)
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
