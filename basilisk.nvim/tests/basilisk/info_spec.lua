--- Tests for basilisk.info — info panel.

describe("basilisk.info", function()
  local info = require("basilisk.info")
  local config_mod = require("basilisk.config")

  after_each(function()
    info.close()
  end)

  describe("show", function()
    it("opens a floating window", function()
      local config = config_mod.resolve()
      info.show(config)
      local found_float = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local win_config = vim.api.nvim_win_get_config(win)
        if win_config.relative and win_config.relative ~= "" then
          found_float = true
          break
        end
      end
      assert.is_true(found_float, "should open a floating window")
    end)

    it("float contains 'Basilisk' text", function()
      local config = config_mod.resolve()
      info.show(config)
      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local win_config = vim.api.nvim_win_get_config(win)
        if win_config.relative and win_config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          for _, line in ipairs(lines) do
            if line:find("Basilisk") then
              found = true
              break
            end
          end
          break
        end
      end
      assert.is_true(found, "should contain 'Basilisk'")
    end)

    it("shows server status", function()
      local config = config_mod.resolve()
      info.show(config)
      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local win_config = vim.api.nvim_win_get_config(win)
        if win_config.relative and win_config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          for _, line in ipairs(lines) do
            if line:find("Status") then
              found = true
              break
            end
          end
          break
        end
      end
      assert.is_true(found, "should show Status line")
    end)

    it("shows analysis mode", function()
      local config = config_mod.resolve()
      info.show(config)
      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local win_config = vim.api.nvim_win_get_config(win)
        if win_config.relative and win_config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          for _, line in ipairs(lines) do
            if line:find("Mode") then
              found = true
              break
            end
          end
          break
        end
      end
      assert.is_true(found, "should show Mode")
    end)

    it("shows integration statuses", function()
      local config = config_mod.resolve()
      info.show(config)
      local found_formatter = false
      local found_uv = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local win_config = vim.api.nvim_win_get_config(win)
        if win_config.relative and win_config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          for _, line in ipairs(lines) do
            if line:find("Formatter") then found_formatter = true end
            if line:find("uv") then found_uv = true end
          end
          break
        end
      end
      assert.is_true(found_formatter, "should show Formatter status")
      assert.is_true(found_uv, "should show uv status")
    end)

    it("closes existing float before opening new one", function()
      local config = config_mod.resolve()
      info.show(config)
      local count_before = 0
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local wc = vim.api.nvim_win_get_config(win)
        if wc.relative and wc.relative ~= "" then count_before = count_before + 1 end
      end
      info.show(config)
      local count_after = 0
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local wc = vim.api.nvim_win_get_config(win)
        if wc.relative and wc.relative ~= "" then count_after = count_after + 1 end
      end
      assert.are.equal(count_before, count_after, "should not accumulate floating windows")
    end)
  end)

  describe("close", function()
    it("closes the floating window", function()
      local config = config_mod.resolve()
      info.show(config)
      info.close()
      local found_float = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local win_config = vim.api.nvim_win_get_config(win)
        if win_config.relative and win_config.relative ~= "" then
          found_float = true
          break
        end
      end
      assert.is_false(found_float, "should close floating window")
    end)

    it("double close does not error", function()
      local config = config_mod.resolve()
      info.show(config)
      info.close()
      assert.has_no.errors(function()
        info.close()
      end)
    end)

    it("close without show does not error", function()
      assert.has_no.errors(function()
        info.close()
      end)
    end)
  end)

  describe("refresh", function()
    it("does not error when panel is not open", function()
      assert.has_no.errors(function()
        local config = config_mod.resolve()
        info.refresh(config)
      end)
    end)

    it("updates content when panel is open", function()
      local config = config_mod.resolve()
      info.show(config)
      assert.has_no.errors(function()
        info.refresh(config)
      end)
    end)
  end)
end)
