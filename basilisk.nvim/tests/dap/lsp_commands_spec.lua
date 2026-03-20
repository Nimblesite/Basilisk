--- DAP LSP command tests — startDebugSession, stopDebugSession, error handling.
---
--- Matches VS Code debug-integration.test.ts tests 1-5, 21-22:
---   LSP command validation, session lifecycle, invalid IDs, bad Python path,
---   multiple simultaneous sessions.

local lsp_helpers = require("tests.lsp.helpers")
local dap_helpers = require("tests.dap.helpers")

local binary = lsp_helpers.find_binary()
if not binary then
  describe("DAP LSP commands (SKIPPED — no binary)", function()
    it("skipped", function()
      pending("basilisk binary not found")
    end)
  end)
  return
end

if not dap_helpers.is_debugpy_installed() then
  describe("DAP LSP commands (SKIPPED — no debugpy)", function()
    it("skipped", function()
      pending("debugpy not installed")
    end)
  end)
  return
end

vim.ui.select = function(items, _, on_choice)
  on_choice(items[1], 1)
end

local tmpdir

describe("DAP LSP commands", function()
  before_each(function()
    tmpdir = lsp_helpers.create_tmpdir()
    local fh = io.open(tmpdir .. "/pyproject.toml", "w")
    assert(fh)
    fh:write('[project]\nname = "test"\nversion = "0.1.0"\n')
    fh:close()
    local fh2 = io.open(tmpdir .. "/hello.py", "w")
    assert(fh2)
    fh2:write('def main() -> None:\n    print("hello")\n')
    fh2:close()
    vim.lsp.config("basilisk", {
      cmd = { binary, "lsp" },
      filetypes = { "python" },
      root_markers = { "pyproject.toml", ".git" },
    })
    vim.lsp.enable("basilisk")
  end)

  after_each(function()
    lsp_helpers.stop_clients()
    lsp_helpers.close_all_buffers()
    lsp_helpers.cleanup_tmpdir(tmpdir)
  end)

  -- ── startDebugSession returns host, port, sessionId ─────────────────

  it("startDebugSession returns host, port, sessionId", function()
    vim.cmd("edit " .. vim.fn.fnameescape(tmpdir .. "/hello.py"))
    local buf = vim.api.nvim_get_current_buf()
    local ready = lsp_helpers.wait_for_server_ready(buf)
    assert.is_true(ready, "LSP server did not become ready")

    local client = lsp_helpers.wait_for_client(buf)
    assert.is_not_nil(client)

    local err, result = lsp_helpers.lsp_request(
      client,
      "workspace/executeCommand",
      { command = "basilisk.startDebugSession", arguments = { {} } },
      buf,
      15000
    )

    assert.is_nil(err, "startDebugSession should not return an error")
    assert.is_not_nil(result, "startDebugSession should return a result")
    assert.is_not_nil(result.host, "result should have host")
    assert.is_true(result.port > 0, "port should be positive")
    assert.is_not_nil(result.sessionId, "result should have sessionId")
    assert.is_truthy(
      result.sessionId:find("^dbg%-"),
      "sessionId should start with dbg-"
    )

    -- Clean up: stop the session.
    lsp_helpers.lsp_request(
      client,
      "workspace/executeCommand",
      {
        command = "basilisk.stopDebugSession",
        arguments = { { sessionId = result.sessionId } },
      },
      buf,
      5000
    )
  end)

  -- ── stopDebugSession kills the debugpy process ──────────────────────

  it("stopDebugSession stops the session", function()
    vim.cmd("edit " .. vim.fn.fnameescape(tmpdir .. "/hello.py"))
    local buf = vim.api.nvim_get_current_buf()
    lsp_helpers.wait_for_server_ready(buf)
    local client = lsp_helpers.wait_for_client(buf)
    assert.is_not_nil(client)

    -- Start a session.
    local _, start_result = lsp_helpers.lsp_request(
      client,
      "workspace/executeCommand",
      { command = "basilisk.startDebugSession", arguments = { {} } },
      buf,
      15000
    )
    assert.is_not_nil(start_result)

    -- Stop it.
    local stop_err, stop_result = lsp_helpers.lsp_request(
      client,
      "workspace/executeCommand",
      {
        command = "basilisk.stopDebugSession",
        arguments = { { sessionId = start_result.sessionId } },
      },
      buf,
      5000
    )

    assert.is_nil(stop_err)
    assert.is_not_nil(stop_result)
    assert.are.equal(true, stop_result.stopped)
  end)

  -- ── stopDebugSession with invalid sessionId ─────────────────────────

  it("stopDebugSession with invalid sessionId returns stopped: false", function()
    vim.cmd("edit " .. vim.fn.fnameescape(tmpdir .. "/hello.py"))
    local buf = vim.api.nvim_get_current_buf()
    lsp_helpers.wait_for_server_ready(buf)
    local client = lsp_helpers.wait_for_client(buf)
    assert.is_not_nil(client)

    local err, result = lsp_helpers.lsp_request(
      client,
      "workspace/executeCommand",
      {
        command = "basilisk.stopDebugSession",
        arguments = { { sessionId = "nonexistent-session-id" } },
      },
      buf,
      5000
    )

    assert.is_nil(err)
    assert.is_not_nil(result)
    assert.are.equal(false, result.stopped)
  end)

  -- ── Multiple simultaneous sessions ──────────────────────────────────

  it("can start multiple debug sessions on different ports", function()
    vim.cmd("edit " .. vim.fn.fnameescape(tmpdir .. "/hello.py"))
    local buf = vim.api.nvim_get_current_buf()
    lsp_helpers.wait_for_server_ready(buf)
    local client = lsp_helpers.wait_for_client(buf)
    assert.is_not_nil(client)

    local _, session1 = lsp_helpers.lsp_request(
      client,
      "workspace/executeCommand",
      { command = "basilisk.startDebugSession", arguments = { {} } },
      buf,
      15000
    )
    assert.is_not_nil(session1)

    local _, session2 = lsp_helpers.lsp_request(
      client,
      "workspace/executeCommand",
      { command = "basilisk.startDebugSession", arguments = { {} } },
      buf,
      15000
    )
    assert.is_not_nil(session2)

    -- Different ports and session IDs.
    assert.are_not.equal(session1.port, session2.port)
    assert.are_not.equal(session1.sessionId, session2.sessionId)

    -- Clean up both.
    lsp_helpers.lsp_request(
      client,
      "workspace/executeCommand",
      {
        command = "basilisk.stopDebugSession",
        arguments = { { sessionId = session1.sessionId } },
      },
      buf,
      5000
    )
    lsp_helpers.lsp_request(
      client,
      "workspace/executeCommand",
      {
        command = "basilisk.stopDebugSession",
        arguments = { { sessionId = session2.sessionId } },
      },
      buf,
      5000
    )
  end)

  -- ── Bad Python path returns error ───────────────────────────────────

  it("startDebugSession with bad Python path returns error", function()
    vim.cmd("edit " .. vim.fn.fnameescape(tmpdir .. "/hello.py"))
    local buf = vim.api.nvim_get_current_buf()
    lsp_helpers.wait_for_server_ready(buf)
    local client = lsp_helpers.wait_for_client(buf)
    assert.is_not_nil(client)

    local err, result = lsp_helpers.lsp_request(
      client,
      "workspace/executeCommand",
      {
        command = "basilisk.startDebugSession",
        arguments = { { python = "/nonexistent/python3.99" } },
      },
      buf,
      15000
    )

    -- Should return an error (debugpy check fails with bad python).
    assert.is_not_nil(err, "expected an error for bad Python path")
    assert.is_nil(result, "should not return a result for bad Python path")
  end)

  -- ── DAP adapter registration ────────────────────────────────────────

  it("DAP adapter is registered after setup", function()
    local dap_ok, dap = pcall(require, "dap")
    if not dap_ok then
      pending("nvim-dap not available")
      return
    end

    require("basilisk.dap").setup({ debugger = { enabled = true }, python = "python3" })
    assert.is_not_nil(dap.adapters.basilisk, "basilisk adapter should be registered")
    assert.is_function(dap.adapters.basilisk, "adapter should be a function")
  end)

  -- ── Default configurations registered ───────────────────────────────

  it("default launch and attach configurations registered", function()
    local dap_ok, dap = pcall(require, "dap")
    if not dap_ok then
      pending("nvim-dap not available")
      return
    end

    -- Clear existing configs to test fresh registration.
    dap.configurations.python = {}
    require("basilisk.dap").setup({ debugger = { enabled = true }, python = "python3" })

    assert.is_true(#dap.configurations.python >= 2, "should have at least 2 configs")
    local has_launch = false
    local has_attach = false
    for _, conf in ipairs(dap.configurations.python) do
      if conf.type == "basilisk" and conf.request == "launch" then
        has_launch = true
      end
      if conf.type == "basilisk" and conf.request == "attach" then
        has_attach = true
      end
    end
    assert.is_true(has_launch, "should have basilisk launch config")
    assert.is_true(has_attach, "should have basilisk attach config")
  end)
end)
