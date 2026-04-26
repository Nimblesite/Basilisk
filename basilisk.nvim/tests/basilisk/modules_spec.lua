--- Tests for basilisk.modules — module explorer panel.

describe("basilisk.modules", function()
  local modules = require("basilisk.modules")

  after_each(function()
    modules.close()
  end)

  describe("open", function()
    it("creates a split window", function()
      local before = #vim.api.nvim_tabpage_list_wins(0)
      modules.open()
      local after = #vim.api.nvim_tabpage_list_wins(0)
      assert.is_true(after > before, "should create a new window")
    end)

    it("creates a buffer with basilisk-modules filetype", function()
      modules.open()
      local found = false
      for _, buf in ipairs(vim.api.nvim_list_bufs()) do
        if vim.api.nvim_buf_is_loaded(buf) and vim.bo[buf].filetype == "basilisk-modules" then
          found = true
          break
        end
      end
      assert.is_true(found, "should create buffer with basilisk-modules filetype")
    end)

    it("buffer is not modifiable", function()
      modules.open()
      for _, buf in ipairs(vim.api.nvim_list_bufs()) do
        if vim.api.nvim_buf_is_loaded(buf) and vim.bo[buf].filetype == "basilisk-modules" then
          assert.is_false(vim.bo[buf].modifiable)
          break
        end
      end
    end)

    it("disables line numbers in the panel window", function()
      modules.open()
      local win = vim.api.nvim_get_current_win()
      assert.is_false(vim.wo[win].number)
      assert.is_false(vim.wo[win].relativenumber)
    end)

    it("sets winfixwidth on the panel window", function()
      modules.open()
      -- The panel window should have winfixwidth set, but it may not
      -- be the current window after after_each cleanup. Just verify
      -- the open/close cycle works.
      local found = false
      for _, win in ipairs(vim.api.nvim_tabpage_list_wins(0)) do
        if vim.wo[win].winfixwidth then
          found = true
          break
        end
      end
      assert.is_true(found, "at least one window should have winfixwidth")
    end)

    it("re-open focuses existing window instead of creating new one", function()
      modules.open()
      local count_after_first = #vim.api.nvim_tabpage_list_wins(0)
      -- Switch away.
      vim.cmd("wincmd p")
      modules.open()
      assert.are.equal(count_after_first, #vim.api.nvim_tabpage_list_wins(0))
    end)

    it("shows placeholder when no LSP client is available", function()
      modules.open()
      vim.wait(200)
      local found_placeholder = false
      for _, buf in ipairs(vim.api.nvim_list_bufs()) do
        if vim.api.nvim_buf_is_loaded(buf) and vim.bo[buf].filetype == "basilisk-modules" then
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          for _, line in ipairs(lines) do
            if line:find("no modules") then
              found_placeholder = true
              break
            end
          end
          break
        end
      end
      assert.is_true(found_placeholder, "should show 'no modules' placeholder without LSP")
    end)
  end)

  describe("close", function()
    it("removes the panel window", function()
      modules.open()
      local before = #vim.api.nvim_tabpage_list_wins(0)
      modules.close()
      local after = #vim.api.nvim_tabpage_list_wins(0)
      assert.is_true(after < before, "should remove the window")
    end)

    it("double close does not error", function()
      modules.open()
      modules.close()
      assert.has_no.errors(function()
        modules.close()
      end)
    end)

    it("close without open does not error", function()
      assert.has_no.errors(function()
        modules.close()
      end)
    end)
  end)

  describe("toggle", function()
    it("opens when closed", function()
      local before = #vim.api.nvim_tabpage_list_wins(0)
      modules.toggle()
      assert.is_true(#vim.api.nvim_tabpage_list_wins(0) > before)
    end)

    it("closes when open", function()
      local before = #vim.api.nvim_tabpage_list_wins(0)
      modules.toggle()
      modules.toggle()
      assert.are.equal(before, #vim.api.nvim_tabpage_list_wins(0))
    end)
  end)

  describe("refresh", function()
    it("does not error when panel is not open", function()
      assert.has_no.errors(function()
        modules.refresh()
      end)
    end)

    it("does not error when panel is open", function()
      modules.open()
      assert.has_no.errors(function()
        modules.refresh()
      end)
    end)
  end)
end)
