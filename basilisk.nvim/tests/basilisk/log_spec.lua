--- Tests for basilisk.log module.
---
--- Covers: level filtering, all log functions, file logging lifecycle,
--- format string handling, and level boundary behavior.

describe("basilisk.log", function()
  local log = require("basilisk.log")

  -- Capture vim.notify calls.
  local notifications = {}
  local orig_notify

  before_each(function()
    notifications = {}
    orig_notify = vim.notify
    vim.notify = function(msg, level)
      notifications[#notifications + 1] = { msg = msg, level = level }
    end
  end)

  after_each(function()
    vim.notify = orig_notify
    log.close_file()
    log.set_level("info")
  end)

  describe("set_level", function()
    it("accepts valid log levels", function()
      for _, level in ipairs({ "trace", "debug", "info", "warn", "error" }) do
        assert.has_no.errors(function()
          log.set_level(level)
        end)
      end
    end)

    it("ignores invalid log levels without error", function()
      assert.has_no.errors(function()
        log.set_level("invalid")
        log.set_level("")
        log.set_level("TRACE")
      end)
    end)
  end)

  describe("level filtering", function()
    it("info level suppresses debug messages", function()
      log.set_level("info")
      log.debug("should not appear")
      assert.are.equal(0, #notifications, "debug should be suppressed at info level")
    end)

    it("info level suppresses trace messages", function()
      log.set_level("info")
      log.trace("should not appear")
      assert.are.equal(0, #notifications, "trace should be suppressed at info level")
    end)

    it("info level allows info messages", function()
      log.set_level("info")
      log.info("visible")
      assert.are.equal(1, #notifications)
      assert.truthy(notifications[1].msg:find("visible"))
    end)

    it("info level allows warn messages", function()
      log.set_level("info")
      log.warn("warning")
      assert.are.equal(1, #notifications)
    end)

    it("info level allows error messages", function()
      log.set_level("info")
      log.error("error")
      assert.are.equal(1, #notifications)
    end)

    it("error level suppresses info and warn", function()
      log.set_level("error")
      log.info("suppressed")
      log.warn("suppressed")
      assert.are.equal(0, #notifications)
    end)

    it("error level allows error messages", function()
      log.set_level("error")
      log.error("visible")
      assert.are.equal(1, #notifications)
    end)

    it("trace level allows all messages", function()
      log.set_level("trace")
      log.trace("t")
      log.debug("d")
      log.info("i")
      log.warn("w")
      log.error("e")
      assert.are.equal(5, #notifications)
    end)
  end)

  describe("format strings", function()
    it("formats string arguments", function()
      log.set_level("info")
      log.info("hello %s", "world")
      assert.truthy(notifications[1].msg:find("hello world"))
    end)

    it("formats numeric arguments", function()
      log.set_level("info")
      log.info("count: %d", 42)
      assert.truthy(notifications[1].msg:find("count: 42"))
    end)

    it("formats multiple arguments", function()
      log.set_level("info")
      log.info("%s has %d items", "list", 3)
      assert.truthy(notifications[1].msg:find("list has 3 items"))
    end)

    it("all messages are prefixed with [basilisk]", function()
      log.set_level("trace")
      log.trace("test")
      log.debug("test")
      log.info("test")
      log.warn("test")
      log.error("test")
      for _, notif in ipairs(notifications) do
        assert.truthy(notif.msg:match("^%[basilisk%]"), "should be prefixed with [basilisk]")
      end
    end)
  end)

  describe("notify levels", function()
    it("trace sends TRACE level", function()
      log.set_level("trace")
      log.trace("msg")
      assert.are.equal(vim.log.levels.TRACE, notifications[1].level)
    end)

    it("debug sends DEBUG level", function()
      log.set_level("debug")
      log.debug("msg")
      assert.are.equal(vim.log.levels.DEBUG, notifications[1].level)
    end)

    it("info sends INFO level", function()
      log.set_level("info")
      log.info("msg")
      assert.are.equal(vim.log.levels.INFO, notifications[1].level)
    end)

    it("warn sends WARN level", function()
      log.set_level("info")
      log.warn("msg")
      assert.are.equal(vim.log.levels.WARN, notifications[1].level)
    end)

    it("error sends ERROR level", function()
      log.set_level("info")
      log.error("msg")
      assert.are.equal(vim.log.levels.ERROR, notifications[1].level)
    end)
  end)

  describe("file logging", function()
    it("writes messages to file", function()
      local tmpfile = vim.fn.tempname() .. ".log"
      log.enable_file(tmpfile)
      log.set_level("info")
      log.info("file log test message")
      log.close_file()

      local fh = io.open(tmpfile, "r")
      assert.is_not_nil(fh, "log file should exist")
      local content = fh:read("*a")
      fh:close()
      assert.truthy(content:find("file log test message"), "file should contain the logged message")
      os.remove(tmpfile)
    end)

    it("includes timestamp in file log", function()
      local tmpfile = vim.fn.tempname() .. ".log"
      log.enable_file(tmpfile)
      log.set_level("info")
      log.info("timestamp test")
      log.close_file()

      local fh = io.open(tmpfile, "r")
      local content = fh:read("*a")
      fh:close()
      assert.truthy(content:match("%d%d%d%d%-%d%d%-%d%d"), "file log should include date")
      os.remove(tmpfile)
    end)

    it("close_file is idempotent", function()
      assert.has_no.errors(function()
        log.close_file()
        log.close_file()
        log.close_file()
      end)
    end)

    it("enable_file closes previous file", function()
      local tmp1 = vim.fn.tempname() .. "_1.log"
      local tmp2 = vim.fn.tempname() .. "_2.log"
      log.enable_file(tmp1)
      log.set_level("info")
      log.info("msg1")
      log.enable_file(tmp2)
      log.info("msg2")
      log.close_file()

      local fh1 = io.open(tmp1, "r")
      local content1 = fh1:read("*a")
      fh1:close()
      assert.truthy(content1:find("msg1"))

      local fh2 = io.open(tmp2, "r")
      local content2 = fh2:read("*a")
      fh2:close()
      assert.truthy(content2:find("msg2"))
      assert.is_falsy(content2:find("msg1"), "msg1 should only be in first file")

      os.remove(tmp1)
      os.remove(tmp2)
    end)

    it("suppressed messages are not written to file", function()
      local tmpfile = vim.fn.tempname() .. ".log"
      log.enable_file(tmpfile)
      log.set_level("error")
      log.info("should not appear")
      log.close_file()

      local fh = io.open(tmpfile, "r")
      local content = fh:read("*a")
      fh:close()
      assert.are.equal("", content, "suppressed messages should not be in file")
      os.remove(tmpfile)
    end)
  end)
end)
