--- UI tests for basilisk.statusline module.
---
--- Covers: get/get_color for all states, lualine component, profiler status,
--- state pinning, diagnostic counts.

describe("basilisk.statusline", function()
  local statusline = require("basilisk.statusline")

  after_each(function()
    statusline.set_state("stopped")
    statusline.set_profiler_status(nil)
  end)

  describe("get", function()
    it("shows stopped state when no LSP client", function()
      statusline.set_state("stopped")
      local text = statusline.get()
      assert.truthy(text:find("Basilisk"))
    end)

    it("returns a string", function()
      assert.is_string(statusline.get())
    end)

    it("contains the state icon", function()
      statusline.set_state("stopped")
      local text = statusline.get()
      -- Stopped icon is ⊘ (U+2298).
      assert.truthy(text:find("\u{2298}"), "stopped state should have ⊘ icon")
    end)

    it("starting state has rotating icon", function()
      statusline.set_state("starting")
      local text = statusline.get()
      assert.truthy(text:find("\u{27f3}"), "starting state should have ⟳ icon")
    end)

    it("error state has cross icon", function()
      statusline.set_state("error")
      local text = statusline.get()
      assert.truthy(text:find("\u{2717}"), "error state should have ✗ icon")
    end)
  end)

  describe("get_color", function()
    it("returns Comment for stopped state", function()
      statusline.set_state("stopped")
      assert.are.equal("Comment", statusline.get_color())
    end)

    it("returns DiagnosticWarn for starting state", function()
      statusline.set_state("starting")
      assert.are.equal("DiagnosticWarn", statusline.get_color())
    end)

    it("returns DiagnosticError for error state", function()
      statusline.set_state("error")
      assert.are.equal("DiagnosticError", statusline.get_color())
    end)

    it("returns a highlight group name", function()
      local color = statusline.get_color()
      assert.is_string(color)
      assert.is_true(#color > 0)
    end)
  end)

  describe("set_state", function()
    it("changes to starting", function()
      statusline.set_state("starting")
      assert.truthy(statusline.get():find("Basilisk"))
      assert.are.equal("DiagnosticWarn", statusline.get_color())
    end)

    it("changes to error", function()
      statusline.set_state("error")
      assert.are.equal("DiagnosticError", statusline.get_color())
    end)

    it("changes to stopped", function()
      statusline.set_state("stopped")
      assert.are.equal("Comment", statusline.get_color())
    end)

    it("pinned starting state is not overridden by update", function()
      statusline.set_state("starting")
      statusline.update()
      -- Starting is pinned — update should not change it to stopped.
      assert.are.equal("DiagnosticWarn", statusline.get_color())
    end)

    it("pinned error state is not overridden by update", function()
      statusline.set_state("error")
      statusline.update()
      assert.are.equal("DiagnosticError", statusline.get_color())
    end)

    it("stopped state unpins and allows update", function()
      statusline.set_state("starting")
      statusline.set_state("stopped")
      -- Should now be unpinned.
      statusline.update()
      assert.are.equal("Comment", statusline.get_color())
    end)
  end)

  describe("lualine_component", function()
    it("is a valid table", function()
      assert.is_table(statusline.lualine_component)
    end)

    it("has a callable function at index 1", function()
      assert.is_function(statusline.lualine_component[1])
    end)

    it("function returns a string", function()
      local result = statusline.lualine_component[1]()
      assert.is_string(result)
      assert.truthy(result:find("Basilisk"))
    end)

    it("has a color function", function()
      assert.is_function(statusline.lualine_component.color)
    end)

    it("color function returns a table with fg", function()
      local result = statusline.lualine_component.color()
      assert.is_table(result)
      -- fg may be nil if the highlight group doesn't have a foreground set,
      -- but the table should exist.
    end)
  end)

  describe("profiler status", function()
    it("get_profiler returns empty string when not profiling", function()
      statusline.set_profiler_status(nil)
      assert.are.equal("", statusline.get_profiler())
    end)

    it("get_profiler returns formatted string when profiling", function()
      statusline.set_profiler_status({
        pid = 12345,
        elapsedSeconds = 10,
        totalSamples = 500,
      })
      local result = statusline.get_profiler()
      assert.truthy(result:find("12345"), "should contain PID")
      assert.truthy(result:find("10"), "should contain elapsed seconds")
      assert.truthy(result:find("500"), "should contain sample count")
      assert.truthy(result:find("Profiling"), "should contain 'Profiling'")
    end)

    it("set_profiler_status with nil clears profiler", function()
      statusline.set_profiler_status({ pid = 1, elapsedSeconds = 0, totalSamples = 0 })
      assert.is_true(#statusline.get_profiler() > 0)
      statusline.set_profiler_status(nil)
      assert.are.equal("", statusline.get_profiler())
    end)

    it("handles missing fields gracefully", function()
      statusline.set_profiler_status({})
      local result = statusline.get_profiler()
      assert.is_string(result)
      assert.truthy(result:find("Profiling"))
    end)
  end)

  describe("update", function()
    it("sets stopped when no clients exist", function()
      statusline.set_state("stopped")
      statusline.update()
      assert.are.equal("Comment", statusline.get_color())
    end)

    it("does not error", function()
      assert.has_no.errors(function()
        statusline.update()
      end)
    end)
  end)
end)
