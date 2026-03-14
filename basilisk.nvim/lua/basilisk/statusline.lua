--- Status line component for Basilisk.
---
--- Compatible with lualine.nvim, heirline.nvim, or any status line
--- that accepts a function returning a string.

local M = {}

---@alias BasiliskState "starting"|"ready"|"error"|"stopped"

--- Current server state.
---@type BasiliskState
local state = "stopped"

--- Cached diagnostic counts.
local error_count = 0
local warn_count = 0

--- State display configuration.
local STATE_DISPLAY = {
  starting = { icon = "\u{27f3}", text = "Basilisk", color = "DiagnosticWarn" },
  ready = { icon = "\u{2713}", text = "Basilisk", color = "DiagnosticOk" },
  error = { icon = "\u{2717}", text = "Basilisk", color = "DiagnosticError" },
  stopped = { icon = "\u{2298}", text = "Basilisk", color = "Comment" },
}

--- Update the cached state from LSP client status.
function M.update()
  local clients = vim.lsp.get_clients({ name = "basilisk" })
  if #clients == 0 then
    state = "stopped"
    error_count = 0
    warn_count = 0
    return
  end

  state = "ready"

  -- Count diagnostics across all buffers.
  local errors = 0
  local warns = 0
  for _, diag in ipairs(vim.diagnostic.get(nil, { namespace = nil })) do
    if diag.source == "basilisk" or (diag.code and tostring(diag.code):match("^BSK")) then
      if diag.severity == vim.diagnostic.severity.ERROR then
        errors = errors + 1
      elseif diag.severity == vim.diagnostic.severity.WARN then
        warns = warns + 1
      end
    end
  end
  error_count = errors
  warn_count = warns
end

--- Get the status line text.
---@return string
function M.get()
  M.update()
  local display = STATE_DISPLAY[state]
  local text = display.icon .. " " .. display.text
  if state == "ready" and (error_count > 0 or warn_count > 0) then
    text = text .. string.format(" (%dE %dW)", error_count, warn_count)
  end
  return text
end

--- Get the highlight group for the current state.
---@return string
function M.get_color()
  M.update()
  local display = STATE_DISPLAY[state]
  if state == "ready" and error_count > 0 then
    return "DiagnosticWarn"
  end
  return display.color
end

--- Lualine-compatible component table.
M.lualine_component = {
  function()
    return M.get()
  end,
  color = function()
    return { fg = vim.api.nvim_get_hl(0, { name = M.get_color() }).fg }
  end,
}

--- Set the state directly (for use by lsp.lua on error/restart).
---@param new_state BasiliskState
function M.set_state(new_state)
  state = new_state
end

return M
