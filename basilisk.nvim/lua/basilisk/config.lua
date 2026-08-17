--- Basilisk configuration defaults and validation.
---
--- All shared LSP settings are defined in LSP-ARCHITECTURE-SPEC.md and forwarded
--- to the server. Neovim-specific settings are documented here.

local log = require("basilisk.log")

local M = {}

---@class BasiliskInlayHints
---@field parameter_names boolean
---@field variable_types boolean


---@class BasiliskDebugger
---@field enabled boolean
---@field type_checking boolean
---@field debugpy_path string

---@class BasiliskTestExplorer
---@field enabled boolean
---@field framework "auto"|"pytest"|"unittest"
---@field pytest_path string
---@field args string[]
---@field auto_discover_on_save boolean
---@field position "left"|"right"|"bottom"
---@field width integer

---@class BasiliskUv
---@field enabled boolean
---@field executable_path? string
---@field auto_sync boolean

---@class BasiliskKeymaps
---@field enabled boolean
---@field prefix string

---@class BasiliskStatusline
---@field enabled boolean

---@class BasiliskConfig
---@field binary_path? string
---@field enabled boolean
---@field use_lsp boolean
---@field analysis_mode "openFilesOnly"|"wholeModule"|"crossModule"
---@field python? string
---@field trace_server "off"|"messages"|"verbose"
---@field inlay_hints BasiliskInlayHints
---@field formatter "ruff"|"none"
---@field debugger BasiliskDebugger
---@field test_explorer BasiliskTestExplorer
---@field uv BasiliskUv
---@field keymaps BasiliskKeymaps
---@field statusline BasiliskStatusline
---@field log_level "trace"|"debug"|"info"|"warn"|"error"

--- Implements [NVIM-NEOVIM-ONLY-CONFIGURATION] — the Neovim-specific settings
--- (keymaps.enabled/prefix, statusline.enabled, test_explorer.position/width,
--- log_level) live here; shared settings are forwarded to the LSP server.
---@type BasiliskConfig
M.defaults = {
  binary_path = nil,
  enabled = true,
  use_lsp = true,
  analysis_mode = "wholeModule",
  python = nil,
  trace_server = "off",
  inlay_hints = {
    parameter_names = true,
    variable_types = true,
  },
  -- Formatter engine ([LSPFMT-CONFIG]): "ruff" is the Ruff formatter embedded
  -- in the basilisk binary (no external ruff needed); "none" disables it.
  formatter = "ruff",
  debugger = {
    enabled = true,
    type_checking = false,
    debugpy_path = "debugpy",
  },
  test_explorer = {
    enabled = true,
    framework = "auto",
    pytest_path = "pytest",
    args = {},
    auto_discover_on_save = true,
    position = "right",
    width = 40,
  },
  uv = {
    enabled = true,
    executable_path = nil,
    auto_sync = false,
  },
  keymaps = {
    enabled = true,
    prefix = "<leader>b",
  },
  statusline = {
    enabled = true,
  },
  log_level = "info",
}

--- Validate the resolved config.
---@param config BasiliskConfig
---@return string[] errors List of validation error messages.
function M.validate(config)
  local errors = {}
  local valid_modes = { openFilesOnly = true, wholeModule = true, crossModule = true }
  if not valid_modes[config.analysis_mode] then
    errors[#errors + 1] = "invalid analysis_mode: " .. tostring(config.analysis_mode)
  end
  local valid_frameworks = { auto = true, pytest = true, unittest = true }
  if not valid_frameworks[config.test_explorer.framework] then
    errors[#errors + 1] = "invalid test_explorer.framework: " .. tostring(config.test_explorer.framework)
  end
  local valid_positions = { left = true, right = true, bottom = true }
  if not valid_positions[config.test_explorer.position] then
    errors[#errors + 1] = "invalid test_explorer.position: " .. tostring(config.test_explorer.position)
  end
  local valid_levels = { trace = true, debug = true, info = true, warn = true, error = true }
  if not valid_levels[config.log_level] then
    errors[#errors + 1] = "invalid log_level: " .. tostring(config.log_level)
  end
  return errors
end

--- Merge user options with defaults and validate.
---@param opts? table
---@return BasiliskConfig
function M.resolve(opts)
  local config = vim.tbl_deep_extend("force", {}, M.defaults, opts or {})
  local errors = M.validate(config)
  for _, err in ipairs(errors) do
    log.error("config error: %s", err)
  end
  return config
end

return M
