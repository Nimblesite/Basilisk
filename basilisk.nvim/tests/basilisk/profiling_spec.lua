--- Tests for basilisk.profiling module.

describe("basilisk.profiling", function()
  local profiling = require("basilisk.profiling")

  describe("display_results", function()
    it("handles nil result gracefully", function()
      assert.has_no.errors(function()
        profiling.display_results(nil)
      end)
      -- Close any floating window.
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          vim.api.nvim_win_close(win, true)
        end
      end
    end)

    it("displays hot functions in floating window", function()
      local result = {
        hotFunctions = {
          { name = "process", file = "/tmp/test.py", line = 10, percentage = 45.2 },
          { name = "calculate", file = "/tmp/test.py", line = 25, percentage = 30.1 },
        },
      }

      assert.has_no.errors(function()
        profiling.display_results(result)
      end)

      -- Find and verify floating window.
      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local text = table.concat(lines, "\n")
          assert.truthy(text:find("process"))
          assert.truthy(text:find("calculate"))
          assert.truthy(text:find("45.2"))
          found = true
          vim.api.nvim_win_close(win, true)
        end
      end
      assert.is_true(found, "should open a floating window with results")
    end)

    it("populates quickfix list", function()
      local result = {
        hotFunctions = {
          { name = "func_a", file = "/tmp/a.py", line = 5, percentage = 60 },
        },
      }

      profiling.display_results(result)
      local qf = vim.fn.getqflist()
      assert.is_true(#qf > 0, "quickfix should have items")

      -- Cleanup.
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          vim.api.nvim_win_close(win, true)
        end
      end
    end)
  end)

  describe("apply_heat_map", function()
    it("handles empty hot functions", function()
      assert.has_no.errors(function()
        profiling.apply_heat_map({})
      end)
    end)

    it("handles nil input", function()
      assert.has_no.errors(function()
        profiling.apply_heat_map(nil)
      end)
    end)
  end)
end)
