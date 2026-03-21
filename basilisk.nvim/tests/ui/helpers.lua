--- Shared UI test helpers for basilisk.nvim.
---
--- Provides utilities for testing floating windows, extmarks,
--- keymaps, and buffer state in headless Neovim.

local M = {}

--- Wait for a condition to be true, with timeout.
---@param condition fun(): boolean
---@param timeout_ms? integer Default 2000.
---@param interval_ms? integer Default 50.
---@return boolean success
function M.wait_for(condition, timeout_ms, interval_ms)
  timeout_ms = timeout_ms or 2000
  interval_ms = interval_ms or 50
  local elapsed = 0
  while elapsed < timeout_ms do
    if condition() then
      return true
    end
    vim.wait(interval_ms)
    elapsed = elapsed + interval_ms
  end
  return false
end

--- Assert that a floating window is open.
---@return integer? win_id The floating window ID, or nil.
function M.find_floating_window()
  for _, win in ipairs(vim.api.nvim_list_wins()) do
    local config = vim.api.nvim_win_get_config(win)
    if config.relative and config.relative ~= "" then
      return win
    end
  end
  return nil
end

--- Assert floating window content contains expected lines.
---@param win integer Window ID.
---@param expected string[] Expected substrings in buffer lines.
---@return boolean
function M.float_contains(win, expected)
  local buf = vim.api.nvim_win_get_buf(win)
  local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
  local text = table.concat(lines, "\n")
  for _, exp in ipairs(expected) do
    if not text:find(exp, 1, true) then
      return false
    end
  end
  return true
end

--- Get all extmarks in a buffer for a given namespace.
---@param buf integer
---@param ns_name string Namespace name.
---@return table[] marks
function M.get_extmarks(buf, ns_name)
  local ns = vim.api.nvim_get_namespaces()[ns_name]
  if not ns then
    return {}
  end
  return vim.api.nvim_buf_get_extmarks(buf, ns, 0, -1, { details = true })
end

--- Get buffer-local keymaps for a given buffer.
---@param buf integer
---@param mode? string Default "n".
---@return table[] maps
function M.get_buf_keymaps(buf, mode)
  mode = mode or "n"
  return vim.api.nvim_buf_get_keymap(buf, mode)
end

--- Check if a specific keymap exists on a buffer.
---@param buf integer
---@param mode string
---@param lhs string
---@return boolean
function M.has_keymap(buf, mode, lhs)
  local maps = M.get_buf_keymaps(buf, mode)
  for _, map in ipairs(maps) do
    if map.lhs == lhs then
      return true
    end
  end
  return false
end

--- Count windows in the current tabpage.
---@return integer
function M.window_count()
  return #vim.api.nvim_tabpage_list_wins(0)
end

--- Get diagnostic count for a buffer in a given namespace.
---@param buf integer
---@param ns_name string
---@return integer
function M.diagnostic_count(buf, ns_name)
  local ns = vim.api.nvim_get_namespaces()[ns_name]
  if not ns then
    return 0
  end
  return #vim.diagnostic.get(buf, { namespace = ns })
end

--- Create a temporary Python buffer with content.
---@param lines string[]
---@return integer buf
function M.create_python_buf(lines)
  local buf = vim.api.nvim_create_buf(true, false)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].filetype = "python"
  return buf
end

return M
