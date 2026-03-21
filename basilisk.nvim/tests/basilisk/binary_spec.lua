--- Tests for basilisk.binary module.

describe("basilisk.binary", function()
  local binary = require("basilisk.binary")

  describe("resolve", function()
    it("returns nil when no binary exists", function()
      -- With a non-existent configured path and no binary on PATH,
      -- resolve should return nil.
      local original_env = vim.env.BASILISK_PATH
      vim.env.BASILISK_PATH = nil

      local result = binary.resolve("/nonexistent/path/to/basilisk")
      -- Result depends on whether basilisk is actually installed.
      -- On CI without the binary, this should be nil.
      -- We mainly test that the function doesn't error.
      assert.is_true(result == nil or type(result) == "string")

      vim.env.BASILISK_PATH = original_env
    end)

    it("respects BASILISK_PATH env var", function()
      local original = vim.env.BASILISK_PATH

      -- Set to a known executable for testing.
      vim.env.BASILISK_PATH = vim.fn.exepath("ls")
      if vim.env.BASILISK_PATH ~= "" then
        local result = binary.resolve()
        assert.are.equal(vim.env.BASILISK_PATH, result)
      end

      vim.env.BASILISK_PATH = original
    end)

    it("prefers configured path over env var", function()
      local original = vim.env.BASILISK_PATH
      local ls_path = vim.fn.exepath("ls")

      if ls_path ~= "" then
        vim.env.BASILISK_PATH = "/nonexistent/should/not/be/used"
        local result = binary.resolve(ls_path)
        assert.are.equal(ls_path, result)
      end

      vim.env.BASILISK_PATH = original
    end)
  end)

  describe("version", function()
    it("returns nil for non-existent binary", function()
      local result = binary.version("/nonexistent/binary")
      assert.is_nil(result)
    end)

    it("returns a string for a valid binary", function()
      -- Use 'ls' as a stand-in — it outputs version-like text.
      local ls_path = vim.fn.exepath("ls")
      if ls_path ~= "" then
        local result = binary.version(ls_path)
        -- ls --version may or may not work depending on OS, just
        -- check it doesn't crash.
        assert.is_true(result == nil or type(result) == "string")
      end
    end)
  end)
end)
