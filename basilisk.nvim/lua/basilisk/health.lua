--- `:checkhealth basilisk` — reports the withdrawal and nothing else.
--- Implements [WITHDRAWAL-SURFACES].

local notice = require("basilisk.notice")

local M = {}

--- @param reporter table|nil injected for tests; defaults to `vim.health`
function M.check(reporter)
  local health = reporter or vim.health
  health.start("basilisk.nvim")
  health.warn("Basilisk is unlisted and its type checker is inert.", notice.lines)
end

return M
