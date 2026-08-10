--- Basilisk for Neovim — a notice. Implements [WITHDRAWAL-SURFACES]; see
--- docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-SURFACES
---
--- The type checker was producing incorrect results, so this plugin starts no
--- language server, registers no command, and configures nothing. It exists so
--- an installed copy tells its owner what happened. The statement itself is
--- generated into notice.lua from the messaging spec and drift-gated in CI.

local notice = require("basilisk.notice")

local M = {}

--- The approved statement, as one string.
function M.notice()
  return notice.text
end

--- Show the statement. A warning, not information: a type checker that stopped
--- checking is a change to the user's setup, not a tip.
function M.announce(notify)
  local emit = notify or vim.notify
  emit(notice.text, vim.log.levels.WARN, { title = "Basilisk is unlisted" })
end

--- Kept so an existing `require('basilisk').setup{...}` does not error on
--- startup. It accepts anything and configures nothing.
function M.setup(_opts, notify)
  M.announce(notify)
end

return M
