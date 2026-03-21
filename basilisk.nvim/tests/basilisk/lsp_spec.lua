--- Tests for basilisk.lsp module.

describe("basilisk.lsp", function()
  local lsp = require("basilisk.lsp")

  describe("restart_count", function()
    it("starts at zero", function()
      assert.are.equal(0, lsp.get_restart_count())
    end)

    it("resets to zero", function()
      lsp.reset_restart_count()
      assert.are.equal(0, lsp.get_restart_count())
    end)
  end)
end)
