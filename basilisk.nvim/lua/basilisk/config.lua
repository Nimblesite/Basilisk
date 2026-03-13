--- Basilisk configuration defaults and validation.
---
--- All shared LSP settings are defined in LSP-SPEC.md and forwarded
--- to the server. Neovim-specific settings are documented here.

local M = {}

---@class BasiliskInlayHints
---@field parameter_names boolean
---@field variable_types boolean

---@class BasiliskRuff
---@field enabled boolean
---@field executable_path string

---@class BasiliskKeymaps
---@field enabled boolean
---@field prefix string

---@class BasiliskStatusline
---@field enabled boolean

---@class BasiliskTestExplorer
---@field position "left"|"right"
---@field width integer

---@class BasiliskConfig
---@field binary_path? string
---@field analysis_mode "openFilesOnly"|"wholeModule"|"crossModule"
---@field python? string
---@field inlay_hints BasiliskInlayHints
---@field ruff BasiliskRuff
---@field keymaps BasiliskKeymaps
---@field statusline BasiliskStatusline
---@field test_explorer BasiliskTestExplorer
---@field log_level "trace"|"debug"|"info"|"warn"|"error"

---@type BasiliskConfig
M.defaults = {
  binary_path = nil,
  analysis_mode = "wholeModule",
  python = nil,
  inlay_hints = {
    parameter_names = true,
    variable_types = true,
  },
  ruff = {
    enabled = true,
    executable_path = "ruff",
  },
  keymaps = {
    enabled = true,
    prefix = "<leader>b",
  },
  statusline = {
    enabled = true,
  },
  test_explorer = {
    position = "right",
    width = 40,
  },
  log_level = "info",
}

--- Merge user options with defaults.
---@param opts? table
---@return BasiliskConfig
function M.resolve(opts)
  return vim.tbl_deep_extend("force", {}, M.defaults, opts or {})
end

return M
