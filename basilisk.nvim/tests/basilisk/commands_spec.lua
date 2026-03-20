--- Tests for basilisk.commands module.

describe("basilisk.commands", function()
  local commands = require("basilisk.commands")
  local config = require("basilisk.config")

  -- Register all commands once with default config.
  local resolved = config.resolve()
  commands.register(resolved)

  --- Helper: assert a user command is registered.
  ---@param name string
  local function assert_command_exists(name)
    local user_commands = vim.api.nvim_get_commands({})
    assert.is_not_nil(user_commands[name], name .. " should be registered")
  end

  describe("core commands", function()
    it("registers BasiliskRestart", function()
      assert_command_exists("BasiliskRestart")
    end)

    it("registers BasiliskInfo", function()
      assert_command_exists("BasiliskInfo")
    end)

    it("registers BasiliskOrganizeImports", function()
      assert_command_exists("BasiliskOrganizeImports")
    end)

    it("registers BasiliskFixFile", function()
      assert_command_exists("BasiliskFixFile")
    end)

    it("registers BasiliskFixWorkspace", function()
      assert_command_exists("BasiliskFixWorkspace")
    end)

    it("registers BasiliskAdoptFile", function()
      assert_command_exists("BasiliskAdoptFile")
    end)

    it("registers BasiliskAdoptWorkspace", function()
      assert_command_exists("BasiliskAdoptWorkspace")
    end)

    it("registers BasiliskUnadoptFile", function()
      assert_command_exists("BasiliskUnadoptFile")
    end)

    it("registers BasiliskShowOutput", function()
      assert_command_exists("BasiliskShowOutput")
    end)
  end)

  describe("refactoring commands", function()
    it("registers BasiliskExtractVariable", function()
      assert_command_exists("BasiliskExtractVariable")
    end)

    it("registers BasiliskExtractConstant", function()
      assert_command_exists("BasiliskExtractConstant")
    end)

    it("registers BasiliskConvertUnion", function()
      assert_command_exists("BasiliskConvertUnion")
    end)

    it("registers BasiliskImplementMethods", function()
      assert_command_exists("BasiliskImplementMethods")
    end)
  end)

  describe("profiling commands", function()
    it("registers BasiliskProfile", function()
      assert_command_exists("BasiliskProfile")
    end)

    it("registers BasiliskProfileStop", function()
      assert_command_exists("BasiliskProfileStop")
    end)

    it("registers BasiliskProfileSnapshot", function()
      assert_command_exists("BasiliskProfileSnapshot")
    end)
  end)

  describe("memory commands", function()
    it("registers BasiliskMemLeak", function()
      assert_command_exists("BasiliskMemLeak")
    end)

    it("registers BasiliskMemStop", function()
      assert_command_exists("BasiliskMemStop")
    end)

    it("registers BasiliskMemRefs", function()
      assert_command_exists("BasiliskMemRefs")
    end)
  end)

  describe("debug commands", function()
    it("registers BasiliskDebugFile", function()
      assert_command_exists("BasiliskDebugFile")
    end)
  end)

  describe("test commands", function()
    it("registers BasiliskTestDiscover", function()
      assert_command_exists("BasiliskTestDiscover")
    end)

    it("registers BasiliskTestRun", function()
      assert_command_exists("BasiliskTestRun")
    end)

    it("registers BasiliskTestDebug", function()
      assert_command_exists("BasiliskTestDebug")
    end)

    it("registers BasiliskTestToggle", function()
      assert_command_exists("BasiliskTestToggle")
    end)
  end)

  describe("uv commands", function()
    it("registers BasiliskUvSync", function()
      assert_command_exists("BasiliskUvSync")
    end)

    it("registers BasiliskUvAdd", function()
      assert_command_exists("BasiliskUvAdd")
    end)

    it("registers BasiliskUvAddDev", function()
      assert_command_exists("BasiliskUvAddDev")
    end)

    it("registers BasiliskUvRemove", function()
      assert_command_exists("BasiliskUvRemove")
    end)

    it("registers BasiliskUvLock", function()
      assert_command_exists("BasiliskUvLock")
    end)

    it("registers BasiliskUvCreateEnv", function()
      assert_command_exists("BasiliskUvCreateEnv")
    end)
  end)

  describe("command descriptions", function()
    it("all commands have descriptions", function()
      local user_commands = vim.api.nvim_get_commands({})
      for name, cmd in pairs(user_commands) do
        if name:match("^Basilisk") then
          assert.is_true(
            cmd.definition ~= nil and cmd.definition ~= "",
            name .. " should have a description"
          )
        end
      end
    end)
  end)
end)
