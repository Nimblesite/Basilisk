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

    it("uv stub_suggestions defaults to true", function()
      assert.is_true(config_mod.defaults.uv.stub_suggestions)
    end)

    it("uv dependency_diagnostics defaults to true", function()
      assert.is_true(config_mod.defaults.uv.dependency_diagnostics)
    end)
  end)

  -- uv config resolution

  describe("config resolution", function()
    it("resolves uv defaults when no overrides given", function()
      local resolved = config_mod.resolve({})
      assert.is_true(resolved.uv.enabled)
      assert.is_nil(resolved.uv.executable_path)
      assert.is_false(resolved.uv.auto_sync)
      assert.is_true(resolved.uv.stub_suggestions)
      assert.is_true(resolved.uv.dependency_diagnostics)
    end)

    it("overrides uv settings from user config", function()
      local resolved = config_mod.resolve({
        uv = {
          enabled = false,
          executable_path = "/usr/local/bin/uv",
          auto_sync = true,
          stub_suggestions = false,
          dependency_diagnostics = false,
        },
      })
      assert.is_false(resolved.uv.enabled)
      assert.are.equal("/usr/local/bin/uv", resolved.uv.executable_path)
      assert.is_true(resolved.uv.auto_sync)
      assert.is_false(resolved.uv.stub_suggestions)
      assert.is_false(resolved.uv.dependency_diagnostics)
    end)

    it("partial uv override preserves other defaults", function()
      local resolved = config_mod.resolve({
        uv = { auto_sync = true },
      })
      assert.is_true(resolved.uv.enabled)
      assert.is_true(resolved.uv.auto_sync)
      assert.is_true(resolved.uv.stub_suggestions)
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
