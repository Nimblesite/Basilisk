--- Health check tests for :checkhealth basilisk.
---
--- Tests [NVIM-HEALTH-CHECK].
---
--- Regression coverage for issue #67: `:checkhealth basilisk` called
--- `binary.resolve()` without the user-configured `binary_path`, so a binary
--- reachable only via `setup({ binary_path = ... })` was falsely reported as
--- "basilisk binary not found" even though the LSP used it all session.
---
--- See https://github.com/Nimblesite/Basilisk/issues/67.
---
--- These tests stub the binary resolver and vim.health, so they run without the
--- real basilisk binary.

local binary = require("basilisk.binary")
local config = require("basilisk.config")
local health = require("basilisk.health")

-- A path that is resolvable ONLY when supplied as the configured path, exactly
-- the scenario from the issue (binary not on PATH or any well-known location).
local CONFIGURED = "/configured/only/path/to/basilisk"

describe("basilisk :checkhealth binary resolution [issue #67]", function()
  local orig_resolve
  local orig_version
  local orig_health
  local orig_config

  before_each(function()
    orig_resolve = binary.resolve
    orig_version = binary.version
    orig_health = vim.health
    orig_config = require("basilisk").config

    -- Simulate a binary reachable only via the configured path: resolve()
    -- with no/other argument finds nothing.
    binary.resolve = function(configured_path)
      if configured_path == CONFIGURED then
        return CONFIGURED
      end
      return nil
    end
    binary.version = function(_)
      return "0.1.0-test"
    end

    -- Configure binary_path like setup({ binary_path = ... }).
    require("basilisk").config = config.resolve({ binary_path = CONFIGURED })
  end)

  after_each(function()
    binary.resolve = orig_resolve
    binary.version = orig_version
    vim.health = orig_health
    require("basilisk").config = orig_config
  end)

  --- Run health.check() with vim.health stubbed; return captured messages.
  local function run_health()
    local messages = { ok = {}, error = {}, warn = {}, info = {}, start = {} }
    vim.health = {
      start = function(s) table.insert(messages.start, s) end,
      ok = function(s) table.insert(messages.ok, s) end,
      error = function(s) table.insert(messages.error, s) end,
      warn = function(s) table.insert(messages.warn, s) end,
      info = function(s) table.insert(messages.info, s) end,
    }
    health.check()
    return messages
  end

  local function any_match(list, needle)
    for _, msg in ipairs(list) do
      if type(msg) == "string" and msg:find(needle, 1, true) then
        return true
      end
    end
    return false
  end

  it("does not report 'binary not found' when binary_path is configured", function()
    local messages = run_health()
    assert.is_false(
      any_match(messages.error, "basilisk binary not found"),
      "health must not report 'binary not found' when a valid binary_path is configured"
    )
  end)

  it("reports the configured binary as found", function()
    local messages = run_health()
    assert.is_true(
      any_match(messages.ok, "basilisk binary found: " .. CONFIGURED),
      "health should report the configured binary as found"
    )
  end)
end)
