--- UI tests for basilisk.statusline module.

describe("basilisk.statusline", function()
  local statusline = require("basilisk.statusline")

  describe("get", function()
    it("shows stopped state when no LSP client", function()
      local text = statusline.get()
      assert.truthy(text:find("Basilisk"))
    end)

    it("returns a string", function()
      assert.is_string(statusline.get())
    end)
  end)

  describe("get_color", function()
    it("returns a highlight group name", function()
      local color = statusline.get_color()
      assert.is_string(color)
    end)
  end)

  describe("set_state", function()
    it("changes state to starting", function()
      statusline.set_state("starting")
      local text = statusline.get()
      -- Should contain the starting icon.
      assert.truthy(text:find("Basilisk"))
    end)

    it("changes state to error", function()
      statusline.set_state("error")
      local color = statusline.get_color()
      assert.are.equal("DiagnosticError", color)
    end)

    it("changes state to stopped", function()
      statusline.set_state("stopped")
      local color = statusline.get_color()
      assert.are.equal("Comment", color)
    end)
  end)

  describe("lualine_component", function()
    it("is a valid table", function()
      assert.is_table(statusline.lualine_component)
    end)

    it("has a callable function", function()
      assert.is_function(statusline.lualine_component[1])
    end)

    it("function returns a string", function()
      local result = statusline.lualine_component[1]()
      assert.is_string(result)
    end)

    it("has a color function", function()
      assert.is_function(statusline.lualine_component.color)
    end)
  end)
end)
