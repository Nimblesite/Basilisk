--- Info panel for Basilisk.
---
--- Displays server status, binary version, Python interpreter, analysis mode,
--- and enabled integrations in a floating window. Follows the same module
--- pattern as `modules.lua` and `type_health.lua`.

local ui = require("basilisk.ui")

local M = {}

---@type integer?
local info_buf = nil
---@type integer?
local info_win = nil

--- Build info lines from config and LSP client state.
---@param config BasiliskConfig
---@return string[]
---@return table[] highlights  { line, col_start, col_end, hl_group }
local function render_info(config)
  local binary_mod = require("basilisk.binary")
  local lsp_mod = require("basilisk.lsp")
  local client = ui.get_client()

  local bin = binary_mod.resolve(config.binary_path)
  local version = bin and binary_mod.version(bin) or "unknown"

  local lines = {
    "Basilisk LSP Server Info",
    "",
  }
  local highlights = {}

  if client then
    lines[#lines + 1] = "  Status:     active"
    highlights[#highlights + 1] = {
      line = #lines - 1,
      col_start = 14,
      col_end = 20,
      hl_group = "DiagnosticOk",
    }
    lines[#lines + 1] = "  Client ID:  " .. tostring(client.id)
    lines[#lines + 1] = "  Root:       " .. (client.root_dir or "nil")
  else
    lines[#lines + 1] = "  Status:     stopped"
    highlights[#highlights + 1] = {
      line = #lines - 1,
      col_start = 14,
      col_end = 21,
      hl_group = "DiagnosticError",
    }
  end

  lines[#lines + 1] = ""
  lines[#lines + 1] = "  Binary:     " .. (bin or "not found")
  lines[#lines + 1] = "  Version:    " .. version
  lines[#lines + 1] = "  Python:     " .. (config.python or "auto-detect")
  lines[#lines + 1] = "  Mode:       " .. config.analysis_mode
  lines[#lines + 1] = "  Restarts:   " .. tostring(lsp_mod.get_restart_count())
  lines[#lines + 1] = ""
  lines[#lines + 1] = "  Ruff:       " .. (config.ruff.enabled and "enabled" or "disabled")
  lines[#lines + 1] = "  Debugger:   " .. (config.debugger.enabled and "enabled" or "disabled")
  lines[#lines + 1] = "  Tests:      " .. (config.test_explorer.enabled and "enabled" or "disabled")
  lines[#lines + 1] = "  uv:         " .. (config.uv.enabled and "enabled" or "disabled")

  return lines, highlights
end

--- Apply highlights to a buffer.
---@param buf integer
---@param highlights table[]
local function apply_highlights(buf, highlights)
  local ns = vim.api.nvim_create_namespace("basilisk_info")
  for _, hl in ipairs(highlights) do
    vim.api.nvim_buf_add_highlight(buf, ns, hl.hl_group, hl.line, hl.col_start, hl.col_end)
  end
end

--- Show the info panel.
---@param config BasiliskConfig
function M.show(config)
  -- Close existing float first.
  if info_win and vim.api.nvim_win_is_valid(info_win) then
    vim.api.nvim_win_close(info_win, true)
  end

  local lines, highlights = render_info(config)
  info_buf, info_win = ui.open_float("Basilisk Info", lines)
  apply_highlights(info_buf, highlights)

  -- Add refresh keybinding.
  vim.keymap.set("n", "r", function()
    M.refresh(config)
  end, { buffer = info_buf })
end

--- Refresh the info panel in-place (if open).
---@param config BasiliskConfig
function M.refresh(config)
  if not info_buf or not vim.api.nvim_buf_is_valid(info_buf) then
    return
  end

  local lines, highlights = render_info(config)
  vim.bo[info_buf].modifiable = true
  vim.api.nvim_buf_set_lines(info_buf, 0, -1, false, lines)
  vim.bo[info_buf].modifiable = false
  apply_highlights(info_buf, highlights)
end

--- Close the info panel.
function M.close()
  if info_win and vim.api.nvim_win_is_valid(info_win) then
    vim.api.nvim_win_close(info_win, true)
  end
  info_buf = nil
  info_win = nil
end

return M
