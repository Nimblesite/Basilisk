--- Tests for basilisk.type_health — type health panel.

describe("basilisk.type_health", function()
  local type_health = require("basilisk.type_health")

  after_each(function()
    type_health.close()
  end)

  describe("open", function()
    it("creates a split window", function()
      local before = #vim.api.nvim_tabpage_list_wins(0)
      type_health.open()
      local after = #vim.api.nvim_tabpage_list_wins(0)
      assert.is_true(after > before, "should create a new window")
    end)

    it("creates a buffer with basilisk-health filetype", function()
      type_health.open()
      local found = false
      for _, buf in ipairs(vim.api.nvim_list_bufs()) do
        if vim.api.nvim_buf_is_loaded(buf) and vim.bo[buf].filetype == "basilisk-health" then
          found = true
          break
        end
      end
      assert.is_true(found, "should create buffer with basilisk-health filetype")
    end)

    it("buffer is not modifiable", function()
      type_health.open()
      for _, buf in ipairs(vim.api.nvim_list_bufs()) do
        if vim.api.nvim_buf_is_loaded(buf) and vim.bo[buf].filetype == "basilisk-health" then
          assert.is_false(vim.bo[buf].modifiable)
          break
        end
      end
    end)

    it("disables line numbers", function()
      type_health.open()
      local win = vim.api.nvim_get_current_win()
      assert.is_false(vim.wo[win].number)
      assert.is_false(vim.wo[win].relativenumber)
    end)

    it("re-open focuses existing window", function()
      type_health.open()
      local count = #vim.api.nvim_tabpage_list_wins(0)
      vim.cmd("wincmd p")
      type_health.open()
      assert.are.equal(count, #vim.api.nvim_tabpage_list_wins(0))
    end)

    it("renders header with coverage info", function()
      type_health.open()
      vim.wait(200)
      for _, buf in ipairs(vim.api.nvim_list_bufs()) do
        if vim.api.nvim_buf_is_loaded(buf) and vim.bo[buf].filetype == "basilisk-health" then
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local has_header = false
          for _, line in ipairs(lines) do
            if line:find("Type Health") then
              has_header = true
              break
            end
          end
          assert.is_true(has_header, "should show Type Health header")
          break
        end
      end
    end)
  end)

  describe("close", function()
    it("removes the panel window", function()
      type_health.open()
      local before = #vim.api.nvim_tabpage_list_wins(0)
      type_health.close()
      assert.is_true(#vim.api.nvim_tabpage_list_wins(0) < before)
    end)

    it("double close does not error", function()
      type_health.open()
      type_health.close()
      assert.has_no.errors(function()
        type_health.close()
      end)
    end)

    it("close without open does not error", function()
      assert.has_no.errors(function()
        type_health.close()
      end)
    end)
  end)

  describe("toggle", function()
    it("opens when closed", function()
      local before = #vim.api.nvim_tabpage_list_wins(0)
      type_health.toggle()
      assert.is_true(#vim.api.nvim_tabpage_list_wins(0) > before)
    end)

    it("closes when open", function()
      local before = #vim.api.nvim_tabpage_list_wins(0)
      type_health.toggle()
      type_health.toggle()
      assert.are.equal(before, #vim.api.nvim_tabpage_list_wins(0))
    end)
  end)

  describe("refresh", function()
    it("does not error when panel is not open", function()
      assert.has_no.errors(function()
        type_health.refresh()
      end)
    end)

    it("does not error when panel is open", function()
      type_health.open()
      assert.has_no.errors(function()
        type_health.refresh()
      end)
    end)
  end)
end)
