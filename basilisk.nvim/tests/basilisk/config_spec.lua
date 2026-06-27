--- Tests for basilisk.config module.
---
--- Tests [NVIM-NEOVIM-ONLY-CONFIGURATION] and the keymap defaults from
--- [NVIM-DEFAULT-KEYMAPS-BASILISK-SPECIFIC] (prefix "<leader>b").

describe("basilisk.config", function()
  local config = require("basilisk.config")

  describe("defaults", function()
    it("has correct analysis_mode default", function()
      assert.are.equal("wholeModule", config.defaults.analysis_mode)
    end)

    it("has inlay hints enabled by default", function()
      assert.is_true(config.defaults.inlay_hints.parameter_names)
      assert.is_true(config.defaults.inlay_hints.variable_types)
    end)

    it("has ruff enabled by default", function()
      assert.is_true(config.defaults.ruff.enabled)
      assert.are.equal("ruff", config.defaults.ruff.executable_path)
    end)

    it("has debugger enabled by default", function()
      assert.is_true(config.defaults.debugger.enabled)
      assert.is_false(config.defaults.debugger.type_checking)
      assert.are.equal("debugpy", config.defaults.debugger.debugpy_path)
    end)

    it("has test explorer with correct defaults", function()
      assert.is_true(config.defaults.test_explorer.enabled)
      assert.are.equal("auto", config.defaults.test_explorer.framework)
      assert.are.equal("pytest", config.defaults.test_explorer.pytest_path)
      assert.are.same({}, config.defaults.test_explorer.args)
      assert.is_true(config.defaults.test_explorer.auto_discover_on_save)
      assert.are.equal("right", config.defaults.test_explorer.position)
      assert.are.equal(40, config.defaults.test_explorer.width)
    end)

    it("has uv enabled by default", function()
      assert.is_true(config.defaults.uv.enabled)
      assert.is_nil(config.defaults.uv.executable_path)
      assert.is_false(config.defaults.uv.auto_sync)
      assert.is_true(config.defaults.uv.stub_suggestions)
      assert.is_true(config.defaults.uv.dependency_diagnostics)
    end)

    it("has keymaps enabled by default", function()
      assert.is_true(config.defaults.keymaps.enabled)
      assert.are.equal("<leader>b", config.defaults.keymaps.prefix)
    end)

    it("has correct log_level default", function()
      assert.are.equal("info", config.defaults.log_level)
    end)
  end)

  describe("resolve", function()
    it("returns defaults when no opts given", function()
      local resolved = config.resolve()
      assert.are.equal("wholeModule", resolved.analysis_mode)
      assert.is_true(resolved.ruff.enabled)
    end)

    it("returns defaults when empty opts given", function()
      local resolved = config.resolve({})
      assert.are.equal("wholeModule", resolved.analysis_mode)
    end)

    it("merges user opts over defaults", function()
      local resolved = config.resolve({
        analysis_mode = "openFilesOnly",
        ruff = { enabled = false },
      })
      assert.are.equal("openFilesOnly", resolved.analysis_mode)
      assert.is_false(resolved.ruff.enabled)
      -- Other defaults preserved.
      assert.are.equal("ruff", resolved.ruff.executable_path)
    end)

    it("deep merges nested tables", function()
      local resolved = config.resolve({
        inlay_hints = { parameter_names = false },
      })
      assert.is_false(resolved.inlay_hints.parameter_names)
      assert.is_true(resolved.inlay_hints.variable_types)
    end)
  end)

  describe("validate", function()
    it("returns no errors for valid config", function()
      local resolved = config.resolve()
      local errors = config.validate(resolved)
      assert.are.equal(0, #errors)
    end)

    it("catches invalid analysis_mode", function()
      local resolved = config.resolve({ analysis_mode = "invalid" })
      local errors = config.validate(resolved)
      assert.are.equal(1, #errors)
      assert.truthy(errors[1]:find("analysis_mode"))
    end)

    it("catches invalid test_explorer.framework", function()
      local resolved = config.resolve({ test_explorer = { framework = "invalid" } })
      local errors = config.validate(resolved)
      assert.are.equal(1, #errors)
      assert.truthy(errors[1]:find("framework"))
    end)

    it("catches invalid test_explorer.position", function()
      local resolved = config.resolve({ test_explorer = { position = "top" } })
      local errors = config.validate(resolved)
      assert.are.equal(1, #errors)
      assert.truthy(errors[1]:find("position"))
    end)

    it("catches invalid log_level", function()
      local resolved = config.resolve({ log_level = "verbose" })
      local errors = config.validate(resolved)
      assert.are.equal(1, #errors)
      assert.truthy(errors[1]:find("log_level"))
    end)
  end)
end)
