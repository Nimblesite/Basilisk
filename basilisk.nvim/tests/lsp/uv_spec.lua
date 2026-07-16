--- Tests for uv integration in basilisk.nvim.
---
--- Validates that uv commands are properly defined, config defaults are
--- correct, and uv settings are passed to the LSP server.

describe("uv integration", function()
  local config_mod

  before_each(function()
    package.loaded["basilisk.config"] = nil
    config_mod = require("basilisk.config")
  end)

  -- uv config defaults

  describe("config defaults", function()
    it("uv is enabled by default", function()
      assert.is_true(config_mod.defaults.uv.enabled)
    end)

    it("uv executable_path defaults to nil (auto-detect)", function()
      assert.is_nil(config_mod.defaults.uv.executable_path)
    end)

    it("uv auto_sync defaults to false", function()
      assert.is_false(config_mod.defaults.uv.auto_sync)
    end)

  end)

  -- uv config resolution

  describe("config resolution", function()
    it("resolves uv defaults when no overrides given", function()
      local resolved = config_mod.resolve({})
      assert.is_true(resolved.uv.enabled)
      assert.is_nil(resolved.uv.executable_path)
      assert.is_false(resolved.uv.auto_sync)
    end)

    it("overrides uv settings from user config", function()
      local resolved = config_mod.resolve({
        uv = {
          enabled = false,
          executable_path = "/usr/local/bin/uv",
          auto_sync = true,
        },
      })
      assert.is_false(resolved.uv.enabled)
      assert.are.equal("/usr/local/bin/uv", resolved.uv.executable_path)
      assert.is_true(resolved.uv.auto_sync)
    end)

    it("partial uv override preserves other defaults", function()
      local resolved = config_mod.resolve({
        uv = { auto_sync = true },
      })
      assert.is_true(resolved.uv.enabled)
      assert.is_true(resolved.uv.auto_sync)
    end)
  end)

  -- uv commands are defined

  describe("commands", function()
    it("registers all uv user commands", function()
      -- Load the commands module to trigger registration.
      package.loaded["basilisk.commands"] = nil
      local commands_mod = require("basilisk.commands")
      local resolved = config_mod.resolve({})

      -- Stub out dependencies that may not be available in test.
      package.loaded["basilisk.lsp"] = {
        reset_restart_count = function() end,
        restart = function() end,
        get_restart_count = function() return 0 end,
      }
      package.loaded["basilisk.profiling"] = {
        start = function() end,
        stop = function() end,
        snapshot = function() end,
      }
      package.loaded["basilisk.memory"] = {
        start = function() end,
        stop = function() end,
        refs = function() end,
        complete_refs = function() return {} end,
      }
      package.loaded["basilisk.testing"] = {
        discover = function() end,
        run = function() end,
        debug = function() end,
        toggle = function() end,
        setup_auto_discover = function() end,
      }

      commands_mod.register(resolved)

      local expected_commands = {
        "BasiliskUvSync",
        "BasiliskUvAdd",
        "BasiliskUvAddDev",
        "BasiliskUvRemove",
        "BasiliskUvLock",
        "BasiliskUvCreateEnv",
      }

      for _, cmd_name in ipairs(expected_commands) do
        local ok, info = pcall(vim.api.nvim_get_commands, { builtin = false })
        if ok and info then
          -- nvim_get_commands returns a table keyed by command name.
          assert.is_not_nil(info[cmd_name], cmd_name .. " should be registered")
        end
      end
    end)
  end)
end)

-- ── Real LSP e2e tests for uv commands ───────────────────────────────────────

local helpers = require("tests.lsp.helpers")

local binary = helpers.find_binary()
if not binary then
  describe("uv commands with real LSP (SKIPPED — no binary)", function()
    it("skipped", function()
      pending("basilisk binary not found")
    end)
  end)
  return
end

local tmpdir

describe("uv commands with real LSP", function()
  before_each(function()
    tmpdir = helpers.create_tmpdir()
    local fh = io.open(tmpdir .. "/pyproject.toml", "w")
    fh:write('[project]\nname = "test"\nversion = "0.1.0"\n')
    fh:close()

    vim.lsp.config("basilisk", {
      cmd = { binary, "lsp" },
      filetypes = { "python" },
      root_markers = { "pyproject.toml", ".git" },
      settings = { basilisk = { analysisMode = "wholeModule" } },
    })
    vim.lsp.enable("basilisk")
  end)

  after_each(function()
    helpers.stop_clients()
    helpers.close_all_buffers()
    helpers.cleanup_tmpdir(tmpdir)
  end)

  --- Helper: register commands and get client.
  local function setup_commands(buf)
    helpers.wait_for_server_ready(buf)
    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)
    return helpers.wait_for_client(buf)
  end

  it(":BasiliskUvSync sends real LSP command", function()
    local buf = helpers.open_python_file(tmpdir, "test_uvsync.py", "x: int = 1\n")
    setup_commands(buf)
    local ok = pcall(vim.cmd, "BasiliskUvSync")
    assert.is_true(ok, ":BasiliskUvSync should not error")
  end)

  it(":BasiliskUvAdd sends real LSP command", function()
    local buf = helpers.open_python_file(tmpdir, "test_uvadd.py", "x: int = 1\n")
    setup_commands(buf)
    local ok = pcall(vim.cmd, "BasiliskUvAdd requests")
    assert.is_true(ok, ":BasiliskUvAdd should not error")
  end)

  it(":BasiliskUvAddDev sends real LSP command", function()
    local buf = helpers.open_python_file(tmpdir, "test_uvadddev.py", "x: int = 1\n")
    setup_commands(buf)
    local ok = pcall(vim.cmd, "BasiliskUvAddDev pytest")
    assert.is_true(ok, ":BasiliskUvAddDev should not error")
  end)

  it(":BasiliskUvRemove sends real LSP command", function()
    local buf = helpers.open_python_file(tmpdir, "test_uvremove.py", "x: int = 1\n")
    setup_commands(buf)
    local ok = pcall(vim.cmd, "BasiliskUvRemove requests")
    assert.is_true(ok, ":BasiliskUvRemove should not error")
  end)

  it(":BasiliskUvLock sends real LSP command", function()
    local buf = helpers.open_python_file(tmpdir, "test_uvlock.py", "x: int = 1\n")
    setup_commands(buf)
    local ok = pcall(vim.cmd, "BasiliskUvLock")
    assert.is_true(ok, ":BasiliskUvLock should not error")
  end)

  it(":BasiliskUvCreateEnv sends real LSP command", function()
    local buf = helpers.open_python_file(tmpdir, "test_uvenv.py", "x: int = 1\n")
    setup_commands(buf)
    local ok = pcall(vim.cmd, "BasiliskUvCreateEnv")
    assert.is_true(ok, ":BasiliskUvCreateEnv should not error")
  end)
end)
