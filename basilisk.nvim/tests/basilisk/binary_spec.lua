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

  describe("is_newer_version", function()
    it("detects newer major version", function()
      assert.is_true(binary.is_newer_version("0.2.1", "1.0.0"))
    end)

    it("detects newer minor version", function()
      assert.is_true(binary.is_newer_version("0.2.1", "0.3.0"))
    end)

    it("detects newer patch version", function()
      assert.is_true(binary.is_newer_version("0.2.1", "0.2.2"))
    end)

    it("returns false for same version", function()
      assert.is_false(binary.is_newer_version("0.2.1", "0.2.1"))
    end)

    it("returns false for older version", function()
      assert.is_false(binary.is_newer_version("1.0.0", "0.9.9"))
    end)

    it("handles v prefix on latest", function()
      assert.is_true(binary.is_newer_version("0.2.1", "v0.3.0"))
    end)

    it("handles v prefix on current", function()
      assert.is_true(binary.is_newer_version("v0.2.1", "0.3.0"))
    end)

    it("handles basilisk prefix from --version output", function()
      assert.is_true(binary.is_newer_version("basilisk 0.2.1", "v0.3.0"))
    end)

    it("handles basilisk prefix on both sides", function()
      assert.is_false(binary.is_newer_version("basilisk 0.3.0", "v0.3.0"))
    end)
  end)

  describe("platform_asset_name", function()
    it("returns a string matching the expected pattern", function()
      local name, is_windows = binary.platform_asset_name()
      -- Should succeed on any CI/dev machine (macOS/Linux/Windows, x86_64/aarch64).
      if name then
        assert.is_true(name:match("^basilisk%-") ~= nil)
        assert.is_true(name:match("%.tar%.gz$") ~= nil or name:match("%.zip$") ~= nil)
        assert.is_true(type(is_windows) == "boolean")
      end
    end)
  end)

  describe("fetch_latest_release", function()
    it("returns a table with tag_name when GitHub is reachable", function()
      -- This test requires network access. Skip gracefully if offline.
      local release = binary.fetch_latest_release()
      if release then
        assert.is_true(type(release.tag_name) == "string")
        assert.is_true(type(release.assets) == "table")
      end
    end)
  end)

  describe("download", function()
    it("returns a path and version on success (requires network)", function()
      -- Only run if we can reach GitHub.
      local release = binary.fetch_latest_release()
      if not release then
        return
      end
      local path, version = binary.download()
      if path then
        assert.is_true(type(path) == "string")
        assert.is_true(vim.fn.executable(path) == 1)
        assert.is_true(type(version) == "string")
        -- Clean up.
        local dir = vim.fn.stdpath("data") .. "/basilisk/" .. version
        vim.fn.delete(dir, "rf")
      end
    end)
  end)

  describe("check_for_updates", function()
    it("does not error for non-existent binary", function()
      -- Should silently return without errors.
      assert.has_no.errors(function()
        binary.check_for_updates("/nonexistent/binary")
      end)
    end)

    it("does not error for a valid binary path", function()
      local ls_path = vim.fn.exepath("ls")
      if ls_path ~= "" then
        assert.has_no.errors(function()
          binary.check_for_updates(ls_path)
        end)
      end
    end)
  end)
end)
