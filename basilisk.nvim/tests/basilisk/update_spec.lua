--- Tests for basilisk.update — :BasiliskUpdate / :BasiliskInstall flows.
---
--- Covers [NVIM-BINARY-UPGRADE]: happy-path update, confirmation gate,
--- already-latest no-op, dev-build and package-manager refusals, install
--- bootstrap, and network-failure handling. All network and LSP calls are
--- stubbed — no test here touches GitHub or a real server.

describe("basilisk.update", function()
  local binary = require("basilisk.binary")
  local lsp = require("basilisk.lsp")
  local update = require("basilisk.update")

  --- Saved originals for everything a test may stub.
  local orig = {}
  --- Notifications captured during a test.
  local notifications
  --- lsp.restart invocations captured during a test.
  local restarts

  before_each(function()
    orig.locate = binary.locate
    orig.version = binary.version
    orig.fetch_latest_release = binary.fetch_latest_release
    orig.download = binary.download
    orig.restart = lsp.restart
    orig.notify = vim.notify
    orig.select = vim.ui.select

    notifications = {}
    vim.notify = function(msg, level)
      notifications[#notifications + 1] = { msg = msg, level = level }
    end

    restarts = {}
    ---@diagnostic disable-next-line: duplicate-set-field
    lsp.restart = function(cfg, force)
      restarts[#restarts + 1] = { cfg = cfg, force = force }
    end

    -- Default: accept the first option ("Update now"/"Install now").
    ---@diagnostic disable-next-line: duplicate-set-field
    vim.ui.select = function(items, _opts, on_choice)
      on_choice(items[1])
    end
  end)

  after_each(function()
    binary.locate = orig.locate
    binary.version = orig.version
    binary.fetch_latest_release = orig.fetch_latest_release
    binary.download = orig.download
    lsp.restart = orig.restart
    vim.notify = orig.notify
    vim.ui.select = orig.select
  end)

  --- Find a captured notification containing `needle`.
  local function notified(needle)
    for _, notif in ipairs(notifications) do
      if notif.msg:find(needle, 1, true) then
        return notif
      end
    end
    return nil
  end

  -- ── update: happy path ───────────────────────────────────────────────────

  describe("update", function()
    it("downloads, rewires binary_path, and restarts the LSP", function()
      binary.locate = function()
        return "/some/manual/place/basilisk"
      end
      binary.version = function()
        return "basilisk 0.1.0"
      end
      binary.fetch_latest_release = function()
        return { tag_name = "v99.0.0", assets = {} }
      end
      local downloaded = false
      binary.download = function()
        downloaded = true
        return "/tmp/fake-cache/v99.0.0/basilisk", "v99.0.0"
      end

      local config = { binary_path = nil }
      update.update(config)

      assert.is_true(downloaded, "should call binary.download()")
      assert.are.equal("/tmp/fake-cache/v99.0.0/basilisk", config.binary_path)
      assert.are.equal(1, #restarts, "should restart the LSP once")
      assert.is_true(restarts[1].force, "restart must bypass the backoff limit")
      assert.is_truthy(notified("v99.0.0"), "should announce the installed version")
    end)

    it("does nothing when the user picks Later", function()
      binary.locate = function()
        return "/some/manual/place/basilisk"
      end
      binary.version = function()
        return "basilisk 0.1.0"
      end
      binary.fetch_latest_release = function()
        return { tag_name = "v99.0.0", assets = {} }
      end
      local downloaded = false
      binary.download = function()
        downloaded = true
        return "/tmp/x/basilisk", "v99.0.0"
      end
      ---@diagnostic disable-next-line: duplicate-set-field
      vim.ui.select = function(_items, _opts, on_choice)
        on_choice(nil) -- user dismissed the prompt
      end

      update.update({ binary_path = nil })

      assert.is_false(downloaded, "declining must not download")
      assert.are.equal(0, #restarts)
    end)

    it("is a no-op when already on the latest version", function()
      binary.locate = function()
        return "/some/manual/place/basilisk"
      end
      binary.version = function()
        return "basilisk 99.99.99"
      end
      binary.fetch_latest_release = function()
        return { tag_name = "v0.1.0", assets = {} }
      end
      local downloaded = false
      binary.download = function()
        downloaded = true
        return nil, nil
      end

      update.update({ binary_path = nil })

      assert.is_false(downloaded)
      assert.are.equal(0, #restarts)
      assert.is_truthy(notified("up to date"), "should say it is already up to date")
    end)

    it("refuses to clobber a Homebrew install", function()
      binary.locate = function()
        return "/opt/homebrew/bin/basilisk"
      end
      local downloaded = false
      binary.download = function()
        downloaded = true
        return nil, nil
      end

      update.update({ binary_path = nil })

      assert.is_false(downloaded)
      assert.is_truthy(notified("brew upgrade basilisk"), "should point at brew")
    end)

    it("refuses to clobber a cargo install", function()
      binary.locate = function()
        return vim.fs.normalize("~/.cargo/bin/basilisk")
      end
      local downloaded = false
      binary.download = function()
        downloaded = true
        return nil, nil
      end

      update.update({ binary_path = nil })

      assert.is_false(downloaded)
      assert.is_truthy(notified("cargo install basilisk-cli"), "should point at cargo")
    end)

    it("refuses to clobber a local dev build", function()
      local tmpfile = vim.fn.tempname()
      local fh = io.open(tmpfile, "w")
      fh:write("#!/bin/sh\necho 'basilisk 0.0.0-PLACEHOLDER'\n")
      fh:close()
      vim.fn.setfperm(tmpfile, "rwxr-xr-x")
      binary.locate = function()
        return tmpfile
      end
      local downloaded = false
      binary.download = function()
        downloaded = true
        return nil, nil
      end

      update.update({ binary_path = nil })
      vim.fn.delete(tmpfile)

      assert.is_false(downloaded)
      assert.is_truthy(notified("dev build"), "should explain it is a dev build")
    end)

    it("reports an error when GitHub is unreachable", function()
      binary.locate = function()
        return "/some/manual/place/basilisk"
      end
      binary.version = function()
        return "basilisk 0.1.0"
      end
      binary.fetch_latest_release = function()
        return nil
      end

      update.update({ binary_path = nil })

      assert.are.equal(0, #restarts)
      local err = notified("latest release")
      assert.is_truthy(err, "should report the fetch failure")
    end)

    it("falls back to the install flow when nothing is installed", function()
      binary.locate = function()
        return nil
      end
      binary.fetch_latest_release = function()
        return { tag_name = "v99.0.0", assets = {} }
      end
      local downloaded = false
      binary.download = function()
        downloaded = true
        return "/tmp/fake-cache/v99.0.0/basilisk", "v99.0.0"
      end

      update.update({ binary_path = nil })

      assert.is_true(downloaded, "update with no install should bootstrap one")
      assert.are.equal(1, #restarts)
    end)
  end)

  -- ── install ──────────────────────────────────────────────────────────────

  describe("install", function()
    it("points at :BasiliskUpdate when a binary already exists", function()
      binary.locate = function()
        return "/some/manual/place/basilisk"
      end
      binary.version = function()
        return "basilisk 0.1.0"
      end
      local downloaded = false
      binary.download = function()
        downloaded = true
        return nil, nil
      end

      update.install({ binary_path = nil })

      assert.is_false(downloaded)
      assert.is_truthy(notified(":BasiliskUpdate"), "should mention :BasiliskUpdate")
    end)

    it("downloads and restarts when nothing is installed", function()
      binary.locate = function()
        return nil
      end
      binary.fetch_latest_release = function()
        return { tag_name = "v99.0.0", assets = {} }
      end
      binary.download = function()
        return "/tmp/fake-cache/v99.0.0/basilisk", "v99.0.0"
      end

      local config = { binary_path = nil }
      update.install(config)

      assert.are.equal("/tmp/fake-cache/v99.0.0/basilisk", config.binary_path)
      assert.are.equal(1, #restarts)
    end)

    it("reports a download failure instead of restarting", function()
      binary.locate = function()
        return nil
      end
      binary.fetch_latest_release = function()
        return { tag_name = "v99.0.0", assets = {} }
      end
      binary.download = function()
        return nil, nil
      end

      update.install({ binary_path = nil })

      assert.are.equal(0, #restarts)
      assert.is_truthy(notified("download failed"), "should report the failure")
    end)
  end)
end)
