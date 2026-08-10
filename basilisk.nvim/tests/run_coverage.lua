--- Coverage exerciser — drives every public entry point of the notice plugin.
---
--- The plugin is a notice ([WITHDRAWAL-SURFACES]): three modules, no server, no
--- adapter. Plenary runs its specs in child processes whose luacov stats do not
--- survive to the parent, so this single-process pass is what produces
--- luacov.stats.out for the threshold gate.
---
--- Usage: LUACOV=1 nvim --headless -u tests/minimal_init.lua -l tests/run_coverage.lua

local basilisk = require("basilisk")
local health = require("basilisk.health")
local notice = require("basilisk.notice")

assert(#notice.lines > 0, "the notice must have content")
assert(notice.text == basilisk.notice(), "the plugin must serve the generated notice")

local announced = 0
basilisk.announce(function()
  announced = announced + 1
end)
basilisk.setup({ anything = true }, function()
  announced = announced + 1
end)
assert(announced == 2, "announce and setup must both emit the notice")

health.check({
  start = function() end,
  warn = function() end,
})

local runner_ok, runner = pcall(require, "luacov.runner")
if runner_ok then
  runner.save_stats()
end
print("coverage exerciser done")
