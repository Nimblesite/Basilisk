--- Activity panel integration tests with the real LSP server.
---
--- Tests :BasiliskModules and :BasiliskHealth commands render correct output.
--- Uses the REAL basilisk binary — no mocking.

local helpers = require("tests.lsp.helpers")

local binary = helpers.find_binary()
if not binary then
  describe("activity panel (SKIPPED — no binary)", function()
    it("skipped", function()
      pending("basilisk binary not found")
    end)
  end)
  return
end

local tmpdir

describe("activity panel with real LSP", function()
  before_each(function()
    tmpdir = helpers.create_tmpdir()
    local fh = io.open(tmpdir .. "/pyproject.toml", "w")
    fh:write('[project]\nname = "test"\nversion = "0.1.0"\n')
    fh:close()

    -- Create multiple Python files so the module tree is non-trivial.
    local files = {
      { name = "alpha.py", content = "def greet(name: str) -> str:\n    return name\n\nx: int = 1\n" },
      { name = "beta.py", content = "class Widget:\n    value: int = 42\n\ny = 'hello'\n" },
      { name = "gamma.py", content = "def add(a: int, b: int) -> int:\n    return a + b\n" },
    }
    for _, file in ipairs(files) do
      local path = tmpdir .. "/" .. file.name
      local f = io.open(path, "w")
      f:write(file.content)
      f:close()
    end

    vim.lsp.config("basilisk", {
      cmd = { binary, "lsp" },
      filetypes = { "python" },
      root_markers = { "pyproject.toml", ".git" },
      settings = { basilisk = { analysisMode = "wholeModule" } },
    })
    vim.lsp.enable("basilisk")
  end)

  after_each(function()
    helpers.stop_clients()
    helpers.close_all_buffers()
    helpers.cleanup_tmpdir(tmpdir)
  end)

  -- ── :BasiliskModules ─────────────────────────────────────────────────────

  it(":BasiliskModules renders correct tree for test workspace", function()
    local buf = helpers.open_python_file(tmpdir, "alpha.py", "def greet(name: str) -> str:\n    return name\n\nx: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    -- Register commands.
    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    -- Execute workspaceModules via LSP to verify data.
    local err, result = helpers.lsp_request(client, "workspace/executeCommand", {
      command = "basilisk.workspaceModules",
      arguments = { {} },
    }, buf, 10000)

    assert.is_nil(err, "workspaceModules should not error")
    assert.is_not_nil(result, "workspaceModules should return data")
    assert.is_not_nil(result.modules, "result should contain modules array")

    local module_names = {}
    for _, mod in ipairs(result.modules) do
      module_names[mod.name] = true
    end

    assert.is_true(module_names["alpha"] ~= nil, "should contain alpha module")
    assert.is_true(module_names["beta"] ~= nil, "should contain beta module")
    assert.is_true(module_names["gamma"] ~= nil, "should contain gamma module")

    -- Verify symbols are present in each module.
    for _, mod in ipairs(result.modules) do
      assert.is_not_nil(mod.symbols, mod.name .. " should have symbols")
      assert.is_true(#mod.symbols > 0, mod.name .. " should have at least one symbol")
      assert.is_not_nil(mod.path, mod.name .. " should have a file path")
      assert.is_not_nil(mod.kind, mod.name .. " should have a kind")
    end

    -- Verify the render_tree function produces correct output.
    local modules_mod = require("basilisk.modules")

    -- Open the module explorer panel.
    vim.cmd("BasiliskModules")
    vim.wait(2000)

    -- Find the modules buffer.
    local modules_buf = nil
    for _, b in ipairs(vim.api.nvim_list_bufs()) do
      if vim.bo[b].filetype == "basilisk-modules" then
        modules_buf = b
        break
      end
    end

    assert.is_not_nil(modules_buf, "should create buffer with basilisk-modules filetype")

    -- Wait for content to render.
    local has_content = helpers.poll_until(function()
      local lines = vim.api.nvim_buf_get_lines(modules_buf, 0, -1, false)
      return #lines > 1 or (lines[1] and lines[1] ~= "" and lines[1] ~= "  (no modules found)")
    end, 5000, "module tree content")

    if has_content then
      local lines = vim.api.nvim_buf_get_lines(modules_buf, 0, -1, false)
      local text = table.concat(lines, "\n")

      -- Module names should appear in the rendered tree.
      assert.truthy(text:find("alpha"), "rendered tree should contain 'alpha'")
      assert.truthy(text:find("beta"), "rendered tree should contain 'beta'")
      assert.truthy(text:find("gamma"), "rendered tree should contain 'gamma'")

      -- Kind labels should appear.
      assert.truthy(text:find("%[mod%]"), "rendered tree should show [mod] labels")
    end

    -- Close the panel.
    modules_mod.close()
  end)

  -- ── :BasiliskHealth ──────────────────────────────────────────────────────

  it(":BasiliskHealth renders correct coverage stats", function()
    local buf = helpers.open_python_file(tmpdir, "alpha.py", "def greet(name: str) -> str:\n    return name\n\nx: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    -- Register commands.
    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    -- Execute typeHealth via LSP to verify data.
    local err, result = helpers.lsp_request(client, "workspace/executeCommand", {
      command = "basilisk.typeHealth",
      arguments = { {} },
    }, buf, 10000)

    assert.is_nil(err, "typeHealth should not error")
    assert.is_not_nil(result, "typeHealth should return data")
    assert.is_not_nil(result.workspace, "result should contain workspace stats")
    assert.is_not_nil(result.modules, "result should contain modules array")

    -- Workspace stats should be present and sane.
    local ws = result.workspace
    assert.is_not_nil(ws.totalSymbols, "should have totalSymbols")
    assert.is_not_nil(ws.annotatedSymbols, "should have annotatedSymbols")
    assert.is_not_nil(ws.coveragePercent, "should have coveragePercent")
    assert.is_true(ws.coveragePercent >= 0 and ws.coveragePercent <= 100, "coverage should be 0-100")
    assert.is_true(ws.totalSymbols >= 3, "should have at least 3 symbols across files")

    -- Module entries should be present.
    assert.is_true(#result.modules >= 3, "should have at least 3 module health entries")
    for _, mod in ipairs(result.modules) do
      assert.is_not_nil(mod.name, "module should have name")
      assert.is_not_nil(mod.coveragePercent, "module should have coveragePercent")
      assert.is_true(mod.coveragePercent >= 0 and mod.coveragePercent <= 100,
        mod.name .. " coverage should be 0-100")
    end

    -- Open the type health panel.
    local type_health = require("basilisk.type_health")
    vim.cmd("BasiliskHealth")
    vim.wait(2000)

    -- Find the health buffer.
    local health_buf = nil
    for _, b in ipairs(vim.api.nvim_list_bufs()) do
      if vim.bo[b].filetype == "basilisk-health" then
        health_buf = b
        break
      end
    end

    assert.is_not_nil(health_buf, "should create buffer with basilisk-health filetype")

    -- Wait for content to render.
    local has_content = helpers.poll_until(function()
      local lines = vim.api.nvim_buf_get_lines(health_buf, 0, -1, false)
      return #lines > 2
    end, 5000, "health panel content")

    if has_content then
      local lines = vim.api.nvim_buf_get_lines(health_buf, 0, -1, false)
      local text = table.concat(lines, "\n")

      -- Header should be present.
      assert.truthy(text:find("Type Health"), "rendered health should contain header")

      -- Coverage information should appear.
      assert.truthy(text:find("Coverage"), "rendered health should show 'Coverage'")
      assert.truthy(text:find("%%"), "rendered health should show percentage")

      -- Symbols count should appear.
      assert.truthy(text:find("Symbols"), "rendered health should show 'Symbols'")
      assert.truthy(text:find("annotated"), "rendered health should show 'annotated'")

      -- Per-module breakdown header should appear.
      assert.truthy(text:find("Per%-Module"), "rendered health should show per-module section")
    end

    -- Close the panel.
    type_health.close()
  end)
end)
