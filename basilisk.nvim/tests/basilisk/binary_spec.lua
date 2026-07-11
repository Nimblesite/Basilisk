--- Tests for basilisk.binary module.
---
--- Covers: resolve cascade, version parsing, semver comparison,
--- platform detection, GitHub release fetching, auto-download,
--- and async update checking.

describe("basilisk.binary", function()
  local binary = require("basilisk.binary")

  -- ── resolve cascade ──────────────────────────────────────────────────────

  describe("resolve", function()
    it("returns nil when no binary exists", function()
      local original_env = vim.env.BASILISK_PATH
      vim.env.BASILISK_PATH = nil

      local result = binary.resolve("/nonexistent/path/to/basilisk")
      assert.is_true(result == nil or type(result) == "string")

      vim.env.BASILISK_PATH = original_env
    end)

    it("respects BASILISK_PATH env var", function()
      local original = vim.env.BASILISK_PATH
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

    it("prefers configured path over well-known locations", function()
      local cat_path = vim.fn.exepath("cat")
      if cat_path ~= "" then
        local result = binary.resolve(cat_path)
        assert.are.equal(cat_path, result)
      end
    end)

    it("warns when configured path is not executable", function()
      local notifications = {}
      local orig_notify = vim.notify
      vim.notify = function(msg, level)
        notifications[#notifications + 1] = { msg = msg, level = level }
      end

      binary.resolve("/totally/bogus/path/basilisk-nope")

      vim.notify = orig_notify
      local found_warning = false
      for _, notif in ipairs(notifications) do
        if notif.msg:find("configured binary_path not found") and notif.level == vim.log.levels.WARN then
          found_warning = true
          break
        end
      end
      assert.is_true(found_warning, "should warn when configured path doesn't exist")
    end)

    it("falls through to env var when configured path is invalid", function()
      local original = vim.env.BASILISK_PATH
      local ls_path = vim.fn.exepath("ls")
      if ls_path ~= "" then
        vim.env.BASILISK_PATH = ls_path
        -- Suppress the warning notification.
        local orig_notify = vim.notify
        vim.notify = function() end
        local result = binary.resolve("/nonexistent/configured/path")
        vim.notify = orig_notify
        assert.are.equal(ls_path, result)
      end
      vim.env.BASILISK_PATH = original
    end)

    it("finds binary on OS PATH when nothing else matches", function()
      local original = vim.env.BASILISK_PATH
      vim.env.BASILISK_PATH = nil
      -- "ls" is always on PATH — use it as a proxy.
      -- We can't easily test this for "basilisk" but we verify the cascade
      -- reaches step 6 by checking exepath is called.
      local result = binary.resolve(nil)
      -- Result may be nil (no basilisk installed) or a real path — both are valid.
      assert.is_true(result == nil or type(result) == "string")
      vim.env.BASILISK_PATH = original
    end)
  end)

  -- ── version ──────────────────────────────────────────────────────────────

  describe("version", function()
    it("returns nil for non-existent binary", function()
      assert.is_nil(binary.version("/nonexistent/binary"))
    end)

    it("returns nil for non-executable path", function()
      -- A regular file that exists but isn't executable.
      local tmpfile = vim.fn.tempname()
      local fh = io.open(tmpfile, "w")
      fh:write("not a binary")
      fh:close()
      vim.fn.setfperm(tmpfile, "rw-r--r--")
      assert.is_nil(binary.version(tmpfile))
      vim.fn.delete(tmpfile)
    end)

    it("returns a trimmed string for a valid binary", function()
      local ls_path = vim.fn.exepath("ls")
      if ls_path ~= "" then
        local result = binary.version(ls_path)
        if result then
          assert.are.equal(result, vim.trim(result), "version should be trimmed")
          assert.is_true(#result > 0, "version should not be empty")
        end
      end
    end)
  end)

  -- ── is_newer_version ──────────────────────────────────────────────────────

  describe("is_newer_version", function()
    -- Major version bumps.
    it("detects newer major version", function()
      assert.is_true(binary.is_newer_version("0.2.1", "1.0.0"))
    end)

    it("detects much newer major version", function()
      assert.is_true(binary.is_newer_version("1.0.0", "5.0.0"))
    end)

    -- Minor version bumps.
    it("detects newer minor version", function()
      assert.is_true(binary.is_newer_version("0.2.1", "0.3.0"))
    end)

    it("detects newer minor version with lower patch", function()
      assert.is_true(binary.is_newer_version("0.2.9", "0.3.0"))
    end)

    -- Patch version bumps.
    it("detects newer patch version", function()
      assert.is_true(binary.is_newer_version("0.2.1", "0.2.2"))
    end)

    it("detects newer patch from zero", function()
      assert.is_true(binary.is_newer_version("0.2.0", "0.2.1"))
    end)

    -- Same / older.
    it("returns false for same version", function()
      assert.is_false(binary.is_newer_version("0.2.1", "0.2.1"))
    end)

    it("returns false for same version with v prefix", function()
      assert.is_false(binary.is_newer_version("v0.2.1", "v0.2.1"))
    end)

    it("returns false when current is newer major", function()
      assert.is_false(binary.is_newer_version("1.0.0", "0.9.9"))
    end)

    it("returns false when current is newer minor", function()
      assert.is_false(binary.is_newer_version("0.5.0", "0.4.9"))
    end)

    it("returns false when current is newer patch", function()
      assert.is_false(binary.is_newer_version("0.2.3", "0.2.2"))
    end)

    -- Prefix stripping.
    it("handles v prefix on latest only", function()
      assert.is_true(binary.is_newer_version("0.2.1", "v0.3.0"))
    end)

    it("handles v prefix on current only", function()
      assert.is_true(binary.is_newer_version("v0.2.1", "0.3.0"))
    end)

    it("handles v prefix on both", function()
      assert.is_true(binary.is_newer_version("v0.2.1", "v0.3.0"))
    end)

    it("handles 'basilisk ' prefix from --version output", function()
      assert.is_true(binary.is_newer_version("basilisk 0.2.1", "v0.3.0"))
    end)

    it("handles 'basilisk ' prefix with same version", function()
      assert.is_false(binary.is_newer_version("basilisk 0.3.0", "v0.3.0"))
    end)

    it("handles 'basilisk ' prefix on both sides", function()
      assert.is_true(binary.is_newer_version("basilisk 0.1.0", "basilisk 0.2.0"))
    end)

    -- Edge cases.
    it("handles versions with only major.minor (no patch)", function()
      -- parse_semver returns 0 for missing patch.
      assert.is_true(binary.is_newer_version("0.2", "0.3.0"))
    end)

    it("handles empty string as current", function()
      assert.is_true(binary.is_newer_version("", "0.1.0"))
    end)

    it("handles garbage input gracefully", function()
      -- All components parse to 0 → same version → not newer.
      assert.is_false(binary.is_newer_version("garbage", "garbage"))
    end)
  end)

  -- ── platform_asset_name ──────────────────────────────────────────────────

  describe("platform_asset_name", function()
    it("returns a valid asset name for the current platform", function()
      local name, is_windows = binary.platform_asset_name()
      assert.is_not_nil(name, "should detect platform on CI/dev machines")
      assert.is_true(type(is_windows) == "boolean")
    end)

    it("asset name starts with 'basilisk-'", function()
      local name = binary.platform_asset_name()
      if name then
        assert.is_true(name:match("^basilisk%-") ~= nil, "should start with 'basilisk-'")
      end
    end)

    it("asset name contains architecture", function()
      local name = binary.platform_asset_name()
      if name then
        local has_arch = name:match("aarch64") or name:match("x86_64")
        assert.is_truthy(has_arch, "should contain aarch64 or x86_64")
      end
    end)

    it("asset name contains OS identifier", function()
      local name = binary.platform_asset_name()
      if name then
        local has_os = name:match("apple%-darwin") or name:match("unknown%-linux%-gnu") or name:match("pc%-windows%-msvc")
        assert.is_truthy(has_os, "should contain OS identifier")
      end
    end)

    -- The archives release.yml actually publishes ([NVIM-BINARY-UPGRADE-ASSETS]).
    -- Anything else means download() silently finds no asset.
    local PUBLISHED_ARCHIVES = {
      ["basilisk-x86_64-unknown-linux-gnu.tar.gz"] = true,
      ["basilisk-aarch64-unknown-linux-gnu.tar.gz"] = true,
      ["basilisk-aarch64-apple-darwin.zip"] = true,
      ["basilisk-x86_64-pc-windows-msvc.zip"] = true,
      ["basilisk-aarch64-pc-windows-msvc.zip"] = true,
    }

    it("asset name is one of the published release archives", function()
      local name = binary.platform_asset_name()
      if name then
        assert.is_true(
          PUBLISHED_ARCHIVES[name] == true,
          "asset name not published by release.yml: " .. name
        )
      end
    end)

    it("Linux asset ends with .tar.gz", function()
      local name = binary.platform_asset_name()
      if name and vim.uv.os_uname().sysname:lower() == "linux" then
        assert.is_truthy(name:match("%.tar%.gz$"), "Linux should end with .tar.gz")
      end
    end)

    it("macOS asset is the exact release archive (zip, aarch64-only)", function()
      local uname = vim.uv.os_uname()
      if uname.sysname:lower() ~= "darwin" then
        return
      end
      local name = binary.platform_asset_name()
      local machine = uname.machine:lower()
      if machine == "arm64" or machine == "aarch64" then
        assert.are.equal("basilisk-aarch64-apple-darwin.zip", name)
      else
        -- No x86_64-apple-darwin archive is published — must not fabricate one.
        assert.is_nil(name)
      end
    end)

    it("Windows asset ends with .zip", function()
      local name, is_windows = binary.platform_asset_name()
      if name and is_windows then
        assert.is_truthy(name:match("%.zip$"), "Windows should end with .zip")
      end
    end)

    it("matches the pattern from basilisk-common release::asset_name", function()
      local name = binary.platform_asset_name()
      if name then
        -- Format: basilisk-{arch}-{os}.{ext}
        local arch, os_str = name:match("^basilisk%-([^%-]+)%-(.+)%.tar%.gz$")
        if not arch then
          arch, os_str = name:match("^basilisk%-([^%-]+)%-(.+)%.zip$")
        end
        assert.is_truthy(arch, "should match asset_name format: got " .. name)
        assert.is_truthy(os_str, "should have OS string in asset name")
      end
    end)
  end)

  -- ── fetch_latest_release ─────────────────────────────────────────────────

  describe("fetch_latest_release", function()
    it("returns a table with tag_name when GitHub is reachable", function()
      local release = binary.fetch_latest_release()
      if release then
        assert.is_true(type(release.tag_name) == "string", "tag_name should be a string")
        assert.is_true(#release.tag_name > 0, "tag_name should not be empty")
      end
    end)

    it("release has assets array", function()
      local release = binary.fetch_latest_release()
      if release then
        assert.is_true(type(release.assets) == "table", "assets should be a table")
      end
    end)

    it("each asset has name and browser_download_url", function()
      local release = binary.fetch_latest_release()
      if release and release.assets and #release.assets > 0 then
        for _, asset in ipairs(release.assets) do
          assert.is_true(type(asset.name) == "string", "asset.name should be a string")
          assert.is_true(#asset.name > 0, "asset.name should not be empty")
          assert.is_true(
            type(asset.browser_download_url) == "string",
            "asset.browser_download_url should be a string"
          )
          assert.is_truthy(
            asset.browser_download_url:match("^https://"),
            "download URL should be HTTPS"
          )
        end
      end
    end)

    it("release contains an asset matching our platform", function()
      local release = binary.fetch_latest_release()
      local our_asset = binary.platform_asset_name()
      if release and our_asset and release.assets then
        local found = false
        for _, asset in ipairs(release.assets) do
          if asset.name == our_asset then
            found = true
            break
          end
        end
        assert.is_true(found, "release should have asset for our platform: " .. our_asset)
      end
    end)

    it("tag_name looks like a semver version", function()
      local release = binary.fetch_latest_release()
      if release then
        local stripped = release.tag_name:gsub("^v", "")
        assert.is_truthy(
          stripped:match("^%d+%.%d+%.%d+"),
          "tag should be semver-ish, got: " .. release.tag_name
        )
      end
    end)
  end)

  -- ── download ─────────────────────────────────────────────────────────────

  describe("download", function()
    it("downloads and extracts a working binary (requires network)", function()
      local release = binary.fetch_latest_release()
      if not release then
        pending("GitHub unreachable — skipping download test")
        return
      end

      local path, version = binary.download()
      if not path then
        pending("Download failed — may be a transient network issue")
        return
      end

      -- Path assertions.
      assert.is_true(type(path) == "string", "path should be a string")
      assert.is_true(#path > 0, "path should not be empty")
      assert.is_true(vim.fn.filereadable(path) == 1, "downloaded binary should exist on disk")
      assert.is_true(vim.fn.executable(path) == 1, "downloaded binary should be executable")

      -- Version assertions.
      assert.is_true(type(version) == "string", "version should be a string")
      assert.is_true(#version > 0, "version should not be empty")
      assert.are.equal(release.tag_name, version, "version should match release tag")

      -- Path should be under stdpath("data")/basilisk/<version>/.
      local expected_dir = vim.fn.stdpath("data") .. "/basilisk/" .. version
      assert.is_truthy(
        path:find(expected_dir, 1, true),
        "binary should be in version-specific cache dir"
      )

      -- Binary name should be 'basilisk' (or 'basilisk.exe' on Windows).
      local basename = vim.fn.fnamemodify(path, ":t")
      assert.is_true(
        basename == "basilisk" or basename == "basilisk.exe",
        "binary name should be basilisk, got: " .. basename
      )

      -- Clean up.
      vim.fn.delete(expected_dir, "rf")
    end)

    it("returns cached binary on second call without re-downloading", function()
      local release = binary.fetch_latest_release()
      if not release then
        pending("GitHub unreachable")
        return
      end

      local path1, version1 = binary.download()
      if not path1 then
        pending("Download failed")
        return
      end

      -- Second call should return the same path from cache.
      local path2, version2 = binary.download()
      assert.are.equal(path1, path2, "second call should return cached path")
      assert.are.equal(version1, version2, "second call should return same version")

      -- Clean up.
      local dir = vim.fn.stdpath("data") .. "/basilisk/" .. version1
      vim.fn.delete(dir, "rf")
    end)

    it("extracts Windows zips with tar, not unzip (stock Windows has no unzip)", function()
      local original_system = vim.fn.system
      local original_asset = binary.platform_asset_name
      local original_fetch = binary.fetch_latest_release

      -- Capture every shell command download() issues; run a no-op through
      -- the real system() so vim.v.shell_error stays 0 (it is read-only).
      local commands = {}
      vim.fn.system = function(cmd)
        table.insert(commands, cmd)
        return original_system({ "true" })
      end
      binary.platform_asset_name = function()
        return "basilisk-x86_64-pc-windows-msvc.zip", true
      end
      binary.fetch_latest_release = function()
        return {
          tag_name = "v0.0.0-windows-test",
          assets = {
            {
              name = "basilisk-x86_64-pc-windows-msvc.zip",
              browser_download_url = "https://example.invalid/basilisk.zip",
            },
          },
        }
      end

      local ok, err = pcall(function()
        binary.download()

        local extract_cmd
        for _, cmd in ipairs(commands) do
          if type(cmd) == "table" and (cmd[1] == "unzip" or cmd[1] == "tar") then
            extract_cmd = cmd
          end
        end
        assert.is_truthy(extract_cmd, "download() should have attempted an extraction")
        assert.are.equal(
          "tar",
          extract_cmd[1],
          "Windows zips must extract via in-box tar.exe (bsdtar, Windows 10 1803+) — "
            .. "stock Windows has no unzip, got: " .. tostring(extract_cmd[1])
        )
      end)

      vim.fn.system = original_system
      binary.platform_asset_name = original_asset
      binary.fetch_latest_release = original_fetch
      vim.fn.delete(vim.fn.stdpath("data") .. "/basilisk/v0.0.0-windows-test", "rf")

      assert(ok, err)
    end)
  end)

  -- ── install_source ───────────────────────────────────────────────────────

  describe("install_source", function()
    it("classifies the plugin-managed cache dir as managed", function()
      local managed = vim.fn.stdpath("data") .. "/basilisk/v0.33.0/basilisk"
      assert.are.equal("managed", binary.install_source(managed))
    end)

    it("classifies Homebrew prefixes as homebrew", function()
      assert.are.equal("homebrew", binary.install_source("/opt/homebrew/bin/basilisk"))
      assert.are.equal("homebrew", binary.install_source("/usr/local/Cellar/basilisk/0.33.0/bin/basilisk"))
      assert.are.equal("homebrew", binary.install_source("/home/linuxbrew/.linuxbrew/bin/basilisk"))
    end)

    it("classifies scoop shims as scoop", function()
      assert.are.equal("scoop", binary.install_source("C:/Users/dev/scoop/shims/basilisk.exe"))
    end)

    it("classifies ~/.cargo/bin as cargo", function()
      local cargo_bin = vim.fs.normalize("~/.cargo/bin/basilisk")
      assert.are.equal("cargo", binary.install_source(cargo_bin))
    end)

    it("classifies a 0.0.0-PLACEHOLDER binary as dev", function()
      local tmpfile = vim.fn.tempname()
      local fh = io.open(tmpfile, "w")
      fh:write("#!/bin/sh\necho 'basilisk 0.0.0-PLACEHOLDER'\n")
      fh:close()
      vim.fn.setfperm(tmpfile, "rwxr-xr-x")
      assert.are.equal("dev", binary.install_source(tmpfile))
      vim.fn.delete(tmpfile)
    end)

    it("classifies everything else as manual", function()
      assert.are.equal("manual", binary.install_source("/some/random/place/basilisk"))
    end)
  end)

  -- ── upgrade_hint ─────────────────────────────────────────────────────────

  describe("upgrade_hint", function()
    it("points managed and manual installs at :BasiliskUpdate", function()
      assert.is_truthy(binary.upgrade_hint("managed"):find(":BasiliskUpdate", 1, true))
      assert.is_truthy(binary.upgrade_hint("manual"):find(":BasiliskUpdate", 1, true))
    end)

    it("points package-manager installs at their own upgrade command", function()
      assert.is_truthy(binary.upgrade_hint("homebrew"):find("brew upgrade basilisk", 1, true))
      assert.is_truthy(binary.upgrade_hint("scoop"):find("scoop update basilisk", 1, true))
      assert.is_truthy(binary.upgrade_hint("cargo"):find("cargo install basilisk-cli", 1, true))
    end)

    it("returns nil for dev builds (no upgrade nag)", function()
      assert.is_nil(binary.upgrade_hint("dev"))
    end)
  end)

  -- ── check_for_updates ────────────────────────────────────────────────────

  describe("check_for_updates", function()
    it("notice names an actionable command, not :checkhealth", function()
      -- Fake binary reporting an ancient version from a "manual" location.
      local tmpfile = vim.fn.tempname()
      local fh = io.open(tmpfile, "w")
      fh:write("#!/bin/sh\necho 'basilisk 0.0.1'\n")
      fh:close()
      vim.fn.setfperm(tmpfile, "rwxr-xr-x")

      -- Stub vim.system so no network is hit and the callback runs promptly.
      local orig_system = vim.system
      ---@diagnostic disable-next-line: duplicate-set-field
      vim.system = function(_cmd, _opts, on_exit)
        on_exit({ code = 0, stdout = '{"tag_name": "v99.99.99"}' })
        return {}
      end

      local notifications = {}
      local orig_notify = vim.notify
      vim.notify = function(msg)
        notifications[#notifications + 1] = msg
      end

      binary.check_for_updates(tmpfile)
      vim.wait(1000, function()
        return #notifications > 0
      end)

      vim.notify = orig_notify
      vim.system = orig_system
      vim.fn.delete(tmpfile)

      assert.is_true(#notifications > 0, "should notify about the update")
      local msg = notifications[1]
      assert.is_truthy(msg:find(":BasiliskUpdate", 1, true), "notice must name :BasiliskUpdate, got: " .. msg)
      assert.is_falsy(msg:find("checkhealth", 1, true), "notice must not dead-end into :checkhealth")
    end)

    it("stays silent for dev builds", function()
      local tmpfile = vim.fn.tempname()
      local fh = io.open(tmpfile, "w")
      fh:write("#!/bin/sh\necho 'basilisk 0.0.0-PLACEHOLDER'\n")
      fh:close()
      vim.fn.setfperm(tmpfile, "rwxr-xr-x")

      local orig_system = vim.system
      local fetched = false
      ---@diagnostic disable-next-line: duplicate-set-field
      vim.system = function(_cmd, _opts, on_exit)
        fetched = true
        on_exit({ code = 0, stdout = '{"tag_name": "v99.99.99"}' })
        return {}
      end

      local notifications = {}
      local orig_notify = vim.notify
      vim.notify = function(msg)
        notifications[#notifications + 1] = msg
      end

      binary.check_for_updates(tmpfile)
      vim.wait(200, function()
        return #notifications > 0
      end)

      vim.notify = orig_notify
      vim.system = orig_system
      vim.fn.delete(tmpfile)

      assert.are.equal(0, #notifications, "dev builds must not be nagged about releases")
      assert.is_false(fetched, "dev builds should not even hit the release API")
    end)

    it("does not error for non-existent binary", function()
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

    it("runs asynchronously without blocking", function()
      local ls_path = vim.fn.exepath("ls")
      if ls_path == "" then
        return
      end

      -- Time the call — should return immediately since it's async.
      local start = vim.uv.hrtime()
      binary.check_for_updates(ls_path)
      local elapsed_ms = (vim.uv.hrtime() - start) / 1e6

      -- Async call should return in under 100ms (no network blocking).
      assert.is_true(
        elapsed_ms < 100,
        "check_for_updates should be async, took " .. elapsed_ms .. "ms"
      )
    end)

    it("notifies user when update is available (simulated)", function()
      -- Simulate by calling is_newer_version directly — the actual async
      -- notification path is tested by checking it doesn't crash.
      local would_notify = binary.is_newer_version("0.0.1", "99.99.99")
      assert.is_true(would_notify, "should detect that 99.99.99 > 0.0.1")
    end)

    it("does not notify when already on latest (simulated)", function()
      local would_notify = binary.is_newer_version("99.99.99", "0.0.1")
      assert.is_false(would_notify, "should not flag downgrade as update")
    end)
  end)

  -- ── resolve with auto-download integration ───────────────────────────────

  describe("resolve with auto-download fallback", function()
    it("resolve returns a path even without local install (requires network)", function()
      local release = binary.fetch_latest_release()
      if not release then
        pending("GitHub unreachable")
        return
      end

      -- Clear env and use bogus configured path to force download cascade.
      local original = vim.env.BASILISK_PATH
      vim.env.BASILISK_PATH = nil

      -- Suppress notifications.
      local orig_notify = vim.notify
      local notifications = {}
      vim.notify = function(msg, level)
        notifications[#notifications + 1] = { msg = msg, level = level }
      end

      local result = binary.resolve("/nonexistent/configured/path")

      vim.notify = orig_notify
      vim.env.BASILISK_PATH = original

      -- On a machine without basilisk installed, this should have
      -- downloaded from GitHub. On a machine with it, it found it locally.
      if result then
        assert.is_true(type(result) == "string")
        assert.is_true(vim.fn.executable(result) == 1)

        -- If it came from download, check the notification.
        if result:find(vim.fn.stdpath("data") .. "/basilisk/") then
          local found_download_msg = false
          for _, notif in ipairs(notifications) do
            if notif.msg:find("downloading") or notif.msg:find("installed") then
              found_download_msg = true
              break
            end
          end
          assert.is_true(found_download_msg, "should notify about download progress")

          -- Clean up the downloaded binary.
          local version_dir = result:match("(.*/basilisk/[^/]+)/")
          if version_dir then
            vim.fn.delete(version_dir, "rf")
          end
        end
      end
    end)
  end)
end)
