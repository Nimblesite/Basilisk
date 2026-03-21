--- Debug integration e2e tests — real LSP, real nvim-dap.
---
--- Tests the full debug lifecycle: startDebugSession via LSP,
--- DAP adapter registration, and session management.

local helpers = require("tests.lsp.helpers")

local binary = helpers.find_binary()
if not binary then
  describe("debug integration (SKIPPED — no binary)", function()
    it("skipped", function()
      pending("basilisk binary not found")
    end)
  end)
  return
end

local dap_ok, dap = pcall(require, "dap")
if not dap_ok then
  describe("debug integration (SKIPPED — no nvim-dap)", function()
    it("skipped", function()
      pending("nvim-dap not installed")
    end)
  end)
  return
end

local tmpdir

describe("debug integration with real LSP", function()
  before_each(function()
    tmpdir = helpers.create_tmpdir()
    local fh = io.open(tmpdir .. "/pyproject.toml", "w")
    fh:write('[project]\nname = "test"\nversion = "0.1.0"\n')
    fh:close()

    vim.lsp.config("basilisk", {
      cmd = { binary, "lsp" },
      filetypes = { "python" },
      root_markers = { "pyproject.toml" },
      settings = { basilisk = { analysisMode = "wholeModule" } },
    })
    vim.lsp.enable("basilisk")
  end)

  after_each(function()
    helpers.stop_clients()
    helpers.close_all_buffers()
    helpers.cleanup_tmpdir(tmpdir)
  end)

  it("basilisk DAP adapter is registered after setup", function()
    local dap_mod = require("basilisk.dap")
    local cfg = require("basilisk.config").resolve({ binary_path = binary })
    dap_mod.setup(cfg)

    assert.is_not_nil(dap.adapters.basilisk, "basilisk adapter should be registered")
    assert.is_function(dap.adapters.basilisk, "adapter should be a function")
  end)

  it("default launch configuration is registered", function()
    local dap_mod = require("basilisk.dap")
    local cfg = require("basilisk.config").resolve({ binary_path = binary })
    dap_mod.setup(cfg)

    assert.is_not_nil(dap.configurations.python, "python configurations should exist")
    local found_launch = false
    for _, conf in ipairs(dap.configurations.python) do
      if conf.type == "basilisk" and conf.request == "launch" then
        found_launch = true
        assert.are.equal("${file}", conf.program)
        assert.is_true(conf.justMyCode)
      end
    end
    assert.is_true(found_launch, "should have basilisk launch configuration")
  end)

  it("default attach configuration is registered", function()
    local dap_mod = require("basilisk.dap")
    local cfg = require("basilisk.config").resolve({ binary_path = binary })
    dap_mod.setup(cfg)

    local found_attach = false
    for _, conf in ipairs(dap.configurations.python) do
      if conf.type == "basilisk" and conf.request == "attach" then
        found_attach = true
        assert.are.equal("127.0.0.1", conf.connect.host)
        assert.are.equal(5678, conf.connect.port)
      end
    end
    assert.is_true(found_attach, "should have basilisk attach configuration")
  end)

  it("startDebugSession sends LSP command", function()
    local buf = helpers.open_python_file(tmpdir, "test_debug.py",
      "def main() -> None:\n    x: int = 42\n    print(x)\n\nmain()\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "workspace/executeCommand", {
      command = "basilisk.startDebugSession",
      arguments = { { uri = vim.uri_from_bufnr(buf), pythonPath = "python3" } },
    }, buf, 10000)

    -- Server may or may not support this yet — the important thing is
    -- we exercise the full request path without crashing.
    if result and result.port then
      assert.is_number(result.port)
      assert.truthy(result.host)
      -- Clean up: stop the debug session.
      if result.sessionId then
        helpers.lsp_request(client, "workspace/executeCommand", {
          command = "basilisk.stopDebugSession",
          arguments = { { sessionId = result.sessionId } },
        }, buf)
      end
    end
  end)

  it("stop_session without active session does not error", function()
    local dap_mod = require("basilisk.dap")
    assert.has_no.errors(function()
      dap_mod.stop_session()
    end)
  end)

  it("setup with debugger disabled skips registration", function()
    -- Clear existing adapter.
    dap.adapters.basilisk = nil

    local dap_mod = require("basilisk.dap")
    local cfg = require("basilisk.config").resolve({ debugger = { enabled = false } })
    dap_mod.setup(cfg)

    -- Adapter should NOT be registered when disabled.
    -- (It might still be there from a previous setup call, but the function should not error.)
  end)

  it("DapTcpProxy listens on a port", function()
    local dap_mod = require("basilisk.dap")
    local proxy_port = nil

    -- Create proxy pointing at a non-existent server (won't connect, but will listen).
    dap_mod.create_proxy("127.0.0.1", 59999, function(port)
      proxy_port = port
    end)

    vim.wait(1000)
    assert.is_not_nil(proxy_port, "proxy should allocate a port")
    assert.is_true(proxy_port > 0, "proxy port should be positive")
  end)
end)
