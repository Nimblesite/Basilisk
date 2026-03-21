--- Shared UI helpers for Basilisk floating windows and LSP client lookup.

local M = {}

--- Get the first active basilisk LSP client, or nil.
---@return vim.lsp.Client?
function M.get_client()
  local clients = vim.lsp.get_clients({ name = "basilisk" })
  return clients[1]
end

--- Open a floating window with the given lines.
---@param title string
---@param lines string[]
---@param filetype? string Buffer filetype (default "basilisk").
---@return integer buf, integer win
function M.open_float(title, lines, filetype)
  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].modifiable = false
  vim.bo[buf].bufhidden = "wipe"
  vim.bo[buf].filetype = filetype or "basilisk"

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

return M
