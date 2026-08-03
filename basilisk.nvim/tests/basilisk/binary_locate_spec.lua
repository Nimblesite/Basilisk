--- Tests for basilisk.binary.locate() — the download-free half of the
--- resolution cascade ([NVIM-BINARY-UPGRADE-INSTALL]).
---
--- Covers the managed-cache scan ([NVIM-BINARY-UPGRADE-MANAGED-DISCOVERY],
--- `lua/basilisk/binary.lua` `newest_managed()` / cascade step 7): a binary the
--- plugin downloaded itself must stay resolvable from disk alone, with no
--- GitHub round trip — issue #370.

describe("basilisk.binary.locate", function()
  local binary = require("basilisk.binary")

  local MANAGED_ROOT = vim.fn.stdpath("data") .. "/basilisk"

  --- Write an executable stub at `<managed root>/<version>/basilisk`.
  ---@param version string
  ---@return string path
  local function install_managed(version)
    local dir = MANAGED_ROOT .. "/" .. version
    local path = dir .. "/basilisk"
    vim.fn.mkdir(dir, "p")
    vim.fn.writefile({ "#!/bin/sh", 'echo "basilisk ' .. version:gsub("^v", "") .. '"' }, path)
    vim.fn.setfperm(path, "rwxr-xr-x")
    return path
  end

  --- Blind every cascade step except the managed cache, and make any network
  --- call a hard failure — locate() is defined as download-free.
  ---@param executable_paths table<string, boolean> Paths that count as executable.
  ---@return function restore
  local function isolate_cascade(executable_paths)
    local originals = {
      executable = vim.fn.executable,
      exepath = vim.fn.exepath,
      env = vim.env.BASILISK_PATH,
      fetch = binary.fetch_latest_release,
    }
    vim.fn.executable = function(path)
      return executable_paths[path] and 1 or 0
    end
    vim.fn.exepath = function()
      return ""
    end
    vim.env.BASILISK_PATH = nil
    binary.fetch_latest_release = function()
      error("locate() must resolve from disk — it must never touch the network")
    end
    return function()
      vim.fn.executable = originals.executable
      vim.fn.exepath = originals.exepath
      vim.env.BASILISK_PATH = originals.env
      binary.fetch_latest_release = originals.fetch
    end
  end

  it("finds a plugin-managed install with no network access (issue #370)", function()
    local version = "v9.99.0"
    local managed = install_managed(version)
    local restore = isolate_cascade({ [managed] = true })

    local ok, result = pcall(binary.locate, nil)

    restore()
    vim.fn.delete(MANAGED_ROOT .. "/" .. version, "rf")

    assert(ok, tostring(result))
    assert.are.equal(
      managed,
      result,
      "a binary the plugin downloaded itself must stay resolvable from disk alone"
    )
  end)

  it("returns the newest managed version when several are installed", function()
    local older = install_managed("v0.9.0")
    local newest = install_managed("v0.10.2")
    local oldest = install_managed("v0.8.7")
    local restore = isolate_cascade({ [older] = true, [newest] = true, [oldest] = true })

    local ok, result = pcall(binary.locate, nil)

    restore()
    for _, version in ipairs({ "v0.9.0", "v0.10.2", "v0.8.7" }) do
      vim.fn.delete(MANAGED_ROOT .. "/" .. version, "rf")
    end

    assert(ok, tostring(result))
    assert.are.equal(newest, result, "0.10.2 beats 0.9.0 — semver, not lexicographic order")
  end)

  it("ignores a version directory left behind by a failed download", function()
    local dir = MANAGED_ROOT .. "/v9.98.0"
    vim.fn.mkdir(dir, "p")
    -- An interrupted extraction leaves the directory without the binary.
    local restore = isolate_cascade({})

    local ok, result = pcall(binary.locate, nil)

    restore()
    vim.fn.delete(dir, "rf")

    assert(ok, tostring(result))
    assert.is_nil(result, "a version dir with no executable is not an install")
  end)

  it("reports nothing installed when the managed cache does not exist", function()
    local stash = MANAGED_ROOT .. ".locate-spec-stash"
    local had_cache = vim.fn.isdirectory(MANAGED_ROOT) == 1
    if had_cache then
      vim.fn.rename(MANAGED_ROOT, stash)
    end
    local restore = isolate_cascade({})

    local ok, result = pcall(binary.locate, nil)

    restore()
    if had_cache then
      vim.fn.delete(MANAGED_ROOT, "rf")
      vim.fn.rename(stash, MANAGED_ROOT)
    end

    assert(ok, "scanning an absent cache dir must not error: " .. tostring(result))
    assert.is_nil(result)
  end)
end)
