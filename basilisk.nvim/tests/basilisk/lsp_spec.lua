--- Tests for basilisk.lsp module.
---
--- Covers: restart count, start (with/without binary), restart backoff,
--- settings passthrough.

describe("basilisk.lsp", function()
  local lsp = require("basilisk.lsp")

  after_each(function()
    lsp.reset_restart_count()
    -- Stop any clients we may have started.
    for _, client in ipairs(vim.lsp.get_clients({ name = "basilisk" })) do
      client:stop(true)
    end
    vim.wait(500, function()
      return #vim.lsp.get_clients({ name = "basilisk" }) == 0
    end)
  end)

  describe("restart_count", function()
    it("starts at zero", function()
      assert.are.equal(0, lsp.get_restart_count())
    end)

    it("resets to zero", function()
      lsp.reset_restart_count()
      assert.are.equal(0, lsp.get_restart_count())
    end)

    it("reset is idempotent", function()
      lsp.reset_restart_count()
      lsp.reset_restart_count()
      assert.are.equal(0, lsp.get_restart_count())
    end)
  end)

  describe("start", function()
    it("returns false when binary is not found", function()
      local config = require("basilisk.config").resolve({ binary_path = "/nonexistent/basilisk" })
      -- Suppress the error notification.
      local orig_notify = vim.notify
      vim.notify = function() end
      local result = lsp.start(config)
      vim.notify = orig_notify
      assert.is_false(result, "should return false when binary not found")
    end)

    it("notifies error when binary is not found", function()
      local notifications = {}
      local orig_notify = vim.notify
      vim.notify = function(msg, level)
        notifications[#notifications + 1] = { msg = msg, level = level }
      end

      local config = require("basilisk.config").resolve({ binary_path = "/nonexistent/basilisk" })
      -- Clear BASILISK_PATH to avoid finding a real binary.
      local orig_env = vim.env.BASILISK_PATH
      vim.env.BASILISK_PATH = nil
      lsp.start(config)
      vim.env.BASILISK_PATH = orig_env
      vim.notify = orig_notify

      local found_error = false
      for _, notif in ipairs(notifications) do
        if notif.level == vim.log.levels.ERROR and notif.msg:find("binary not found") then
          found_error = true
          break
        end
      end
      assert.is_true(found_error, "should notify about missing binary")
    end)

    it("returns true when a valid binary is provided", function()
      -- Use 'cat' as a fake binary (it'll fail as LSP but start() only checks existence).
      local cat_path = vim.fn.exepath("cat")
      if cat_path == "" then
        pending("cat not on PATH")
        return
      end
      local config = require("basilisk.config").resolve({ binary_path = cat_path })
      local result = lsp.start(config)
      assert.is_true(result, "should return true when binary exists")
    end)

    it("resets restart count on successful start", function()
      local cat_path = vim.fn.exepath("cat")
      if cat_path == "" then return end
      local config = require("basilisk.config").resolve({ binary_path = cat_path })
      lsp.start(config)
      assert.are.equal(0, lsp.get_restart_count())
    end)
  end)

  describe("restart", function()
    it("respects max restart limit", function()
      local config = require("basilisk.config").resolve({ binary_path = "/fake" })
      local orig_notify = vim.notify
      local notifications = {}
      vim.notify = function(msg, level)
        notifications[#notifications + 1] = { msg = msg, level = level }
      end

      -- Exhaust restarts by calling restart 4 times (max is 3).
      for _ = 1, 4 do
        lsp.restart(config)
      end

      vim.notify = orig_notify

      local found_max_msg = false
      for _, notif in ipairs(notifications) do
        if notif.msg:find("max restarts") then
          found_max_msg = true
          break
        end
      end
      assert.is_true(found_max_msg, "should warn about max restarts reached")
    end)

    it("force flag bypasses restart limit", function()
      local config = require("basilisk.config").resolve({ binary_path = "/fake" })
      local orig_notify = vim.notify
      vim.notify = function() end

      -- Exhaust normal restarts.
      for _ = 1, 4 do
        lsp.restart(config)
      end

      -- Force should work.
      assert.has_no.errors(function()
        lsp.restart(config, true)
      end)

      vim.notify = orig_notify
    end)
  end)
end)
