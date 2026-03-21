--- Tests for basilisk.memory module.

describe("basilisk.memory", function()
  local memory = require("basilisk.memory")

  describe("display_leak_report", function()
    it("handles nil result gracefully", function()
      assert.has_no.errors(function()
        memory.display_leak_report(nil)
      end)
      -- Close float.
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          vim.api.nvim_win_close(win, true)
        end
      end
    end)

    it("displays leaks in floating window", function()
      local result = {
        leaks = {
          { typeName = "DataFrame", count = 15, totalSize = "1.2MB", location = { file = "/tmp/test.py", line = 42 } },
          { typeName = "dict", count = 100, totalSize = "500KB" },
        },
      }

      assert.has_no.errors(function()
        memory.display_leak_report(result)
      end)

      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local text = table.concat(lines, "\n")
          assert.truthy(text:find("DataFrame"))
          assert.truthy(text:find("dict"))
          assert.truthy(text:find("15 objects"))
          found = true
          vim.api.nvim_win_close(win, true)
        end
      end
      assert.is_true(found, "should open floating window with leak report")
    end)
  end)

  describe("display_retention_paths", function()
    it("handles nil result gracefully", function()
      assert.has_no.errors(function()
        memory.display_retention_paths("dict", nil)
      end)
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          vim.api.nvim_win_close(win, true)
        end
      end
    end)

    it("displays retention paths", function()
      local result = {
        retentionPaths = {
          {
            confidence = 0.85,
            steps = {
              { name = "global_cache", kind = "variable" },
              { name = "__dict__", kind = "attribute" },
            },
          },
        },
      }

      assert.has_no.errors(function()
        memory.display_retention_paths("DataFrame", result)
      end)

      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local text = table.concat(lines, "\n")
          assert.truthy(text:find("DataFrame"))
          assert.truthy(text:find("global_cache"))
          assert.truthy(text:find("85%%"))
          found = true
          vim.api.nvim_win_close(win, true)
        end
      end
      assert.is_true(found)
    end)
  end)

  describe("complete_refs", function()
    it("returns DataFrame for 'Data' input", function()
      local matches = memory.complete_refs("Data")
      assert.are.equal("DataFrame", matches[1])
    end)

    it("returns all types for empty input", function()
      local matches = memory.complete_refs("")
      assert.is_true(#matches >= 10, "should return many type suggestions")
    end)

    it("returns dict for 'dic' input", function()
      local matches = memory.complete_refs("dic")
      local found = false
      for _, m in ipairs(matches) do
        if m == "dict" then
          found = true
        end
      end
      assert.is_true(found)
    end)

    it("is case-insensitive", function()
      local matches = memory.complete_refs("tensor")
      local found = false
      for _, m in ipairs(matches) do
        if m == "Tensor" then
          found = true
        end
      end
      assert.is_true(found)
    end)
  end)
end)
