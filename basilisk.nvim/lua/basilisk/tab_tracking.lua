--- Tab tracking for openFilesOnly analysis mode.
---
--- In openFilesOnly mode, diagnostics should clear when files close.
--- Neovim doesn't reliably fire didClose when a buffer is hidden, so
--- we track buffer visibility and send didClose manually.

local log = require("basilisk.log")
local ui = require("basilisk.ui")

local M = {}

--- Set of URIs we know are visible in windows.
---@type table<string, boolean>
local known_open_uris = {}

--- Collect all Python file URIs currently visible in windows.
---@return table<string, boolean>
local function collect_visible_python_uris()
  local uris = {}
  for _, win in ipairs(vim.api.nvim_list_wins()) do
    local buf = vim.api.nvim_win_get_buf(win)
    if vim.bo[buf].filetype == "python" then
      local name = vim.api.nvim_buf_get_name(buf)
      if name ~= "" then
        uris[vim.uri_from_fname(name)] = true
      end
    end
  end
  return uris
end

--- Check for closed Python tabs and send didClose.
---@param config BasiliskConfig
local function check_closed_tabs(config)
  if config.analysis_mode ~= "openFilesOnly" then
    return
  end

  local client = ui.get_client()
  if not client then
    return
  end

  local current_uris = collect_visible_python_uris()

  -- Find URIs that were open but are no longer visible.
  for uri in pairs(known_open_uris) do
    if not current_uris[uri] then
      client:notify("textDocument/didClose", {
        textDocument = { uri = uri },
      })
      log.debug("sent didClose for hidden buffer: %s", uri)
    end
  end

  known_open_uris = current_uris
end

--- Set up tab tracking autocmds.
---@param config BasiliskConfig
function M.setup(config)
  if config.analysis_mode ~= "openFilesOnly" then
    return
  end

  local group = vim.api.nvim_create_augroup("BasiliskTabTracking", { clear = true })

  -- Track when buffers become hidden or windows change.
  vim.api.nvim_create_autocmd({ "BufHidden", "WinClosed", "BufDelete" }, {
    group = group,
    pattern = "*.py",
    callback = function()
      vim.defer_fn(function()
        check_closed_tabs(config)
      end, 100)
    end,
  })

  -- Seed with currently visible buffers.
  known_open_uris = collect_visible_python_uris()

  log.debug("tab tracking enabled for openFilesOnly mode")
end

return M
