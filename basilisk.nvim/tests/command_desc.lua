--- Read a user command's description across the Neovim versions CI covers.
---
--- Supports [NVIM-DISTRIBUTION-CI]: the `test-nvim` matrix spans a Neovim
--- release boundary, and `nvim_get_commands` reports a Lua-callback command's
--- `desc` differently on either side of it:
---
---   * 0.11 / 0.12 — `definition` carries the description, `desc` is nil.
---   * 0.13-dev    — `definition` is the empty string, `desc` carries it.
---
--- A spec that reads only `definition` therefore does not check the
--- description at all on the nightly leg; it compares "" against its
--- non-empty assertion and fails every command that has a perfectly good
--- `desc`. Reading only `desc` fails the 0.11 leg the same way. Every spec
--- asserting on command descriptions goes through here so the requirement is
--- stated once and holds on both legs.

local M = {}

--- The description Neovim reports for one `nvim_get_commands` entry.
--- @param entry table one value from `vim.api.nvim_get_commands({})`
--- @return string|nil description, or nil when the command genuinely has none
function M.of(entry)
  if entry == nil then
    return nil
  end
  local text = entry.desc
  if text == nil or text == "" then
    text = entry.definition
  end
  if text == nil or text == "" then
    return nil
  end
  return text
end

--- The description for one command name, or nil if the command is absent.
--- @param name string user command name, e.g. "BasiliskInfo"
--- @return string|nil
function M.for_command(name)
  return M.of(vim.api.nvim_get_commands({})[name])
end

return M
