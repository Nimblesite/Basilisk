--- Tests for basilisk.tab_tracking module.

describe("basilisk.tab_tracking", function()
  local tab_tracking = require("basilisk.tab_tracking")
  local config_mod = require("basilisk.config")

  describe("setup", function()
    it("does nothing for wholeModule mode", function()
      local config = config_mod.resolve({ analysis_mode = "wholeModule" })
      assert.has_no.errors(function()
        tab_tracking.setup(config)
      end)
    end)

    it("does nothing for crossModule mode", function()
      local config = config_mod.resolve({ analysis_mode = "crossModule" })
      assert.has_no.errors(function()
        tab_tracking.setup(config)
      end)
    end)

    it("sets up autocmds for openFilesOnly mode", function()
      local config = config_mod.resolve({ analysis_mode = "openFilesOnly" })
      assert.has_no.errors(function()
        tab_tracking.setup(config)
      end)
      -- Verify the augroup was created.
      local groups = vim.api.nvim_get_autocmds({ group = "BasiliskTabTracking" })
      assert.is_true(#groups > 0, "should create autocmds for tab tracking")
    end)
  end)
end)
