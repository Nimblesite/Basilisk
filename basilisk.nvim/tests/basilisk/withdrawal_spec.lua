-- Tests for [WITHDRAWAL-SURFACES]. The plugin's whole contract: it states the
-- approved message and registers nothing.

local basilisk = require("basilisk")
local health = require("basilisk.health")
local notice = require("basilisk.notice")

describe("basilisk.nvim is a notice", function()
  it("carries the approved statement", function()
    assert.truthy(notice.text:find("Basilisk is unlisted%."))
    assert.truthy(notice.text:find("checks nothing", 1, true))
    assert.truthy(notice.text:find("python/typing/pull/2330", 1, true))
    assert.truthy(notice.text:find("basilisk%-conformance%-apology"))
    assert.equals(notice.text, basilisk.notice())
  end)

  it("announces as a warning", function()
    local seen = {}
    basilisk.announce(function(message, level, opts)
      seen = { message = message, level = level, opts = opts }
    end)
    assert.equals(notice.text, seen.message)
    assert.equals(vim.log.levels.WARN, seen.level)
    assert.equals("Basilisk is unlisted", seen.opts.title)
  end)

  it("accepts a legacy setup call without configuring anything", function()
    local announced = 0
    basilisk.setup({ cmd = { "basilisk", "lsp" } }, function()
      announced = announced + 1
    end)
    assert.equals(1, announced)
  end)

  it("reports the withdrawal in checkhealth", function()
    local started, warned = nil, nil
    health.check({
      start = function(name)
        started = name
      end,
      warn = function(message, advice)
        warned = { message = message, advice = advice }
      end,
    })
    assert.equals("basilisk.nvim", started)
    assert.truthy(warned.message:find("inert", 1, true))
    assert.equals(notice.lines[1], warned.advice[1])
  end)

  it("registers no LSP client, command or debug adapter", function()
    assert.equals(0, #vim.lsp.get_clients())
    for _, name in ipairs({ "Basilisk", "BasiliskCheck", "BasiliskRestart", "BasiliskInfo" }) do
      assert.is_nil(vim.fn.exists(":" .. name) == 2 or nil)
    end
    for _, module in ipairs({ "basilisk.lsp", "basilisk.dap", "basilisk.commands", "basilisk.binary" }) do
      assert.is_false(pcall(require, module))
    end
  end)
end)
