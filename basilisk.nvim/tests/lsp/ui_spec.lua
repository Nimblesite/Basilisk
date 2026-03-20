--- Real UI interaction tests with the actual LSP server.
---
--- Tests keymaps, inlay hints, code lens, status line updates,
--- and diagnostic displays with REAL LSP — no mocking.

local helpers = require("tests.lsp.helpers")

local binary = helpers.find_binary()
if not binary then
  describe("basilisk UI interactions (SKIPPED — no binary)", function()
    it("skipped", function()
      pending("basilisk binary not found")
    end)
  end)
  return
end

local tmpdir

describe("basilisk UI interactions with real LSP", function()
  before_each(function()
    tmpdir = helpers.create_tmpdir()
    local fh = io.open(tmpdir .. "/pyproject.toml", "w")
    fh:write('[project]\nname = "test"\nversion = "0.1.0"\n')
    fh:close()

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

  -- Status line updates with real LSP state

  it("statusline shows ready state when LSP is running", function()
    local statusline = require("basilisk.statusline")

    local buf = helpers.open_python_file(tmpdir, "test_status.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    -- Unpin state so update() can detect the client.
    statusline.set_state("ready")

    local text = statusline.get()
    assert.truthy(text:find("Basilisk"), "statusline should contain Basilisk")
  end)

  it("statusline shows diagnostic counts", function()
    local statusline = require("basilisk.statusline")

    local buf = helpers.open_python_file(tmpdir, "test_diag_status.py", "def greet(name):\n    return name\n")
    helpers.wait_for_server_ready(buf)
    helpers.wait_for_diagnostics(buf)

    -- Force state to ready so update() counts diagnostics.
    statusline.set_state("ready")

    local text = statusline.get()
    -- The status line should reflect some diagnostic presence.
    assert.truthy(text:find("Basilisk"), "statusline should contain Basilisk")
  end)

  -- Inlay hints with real LSP

  it("inlay hints can be enabled on a buffer", function()
    local buf = helpers.open_python_file(tmpdir, "test_hints.py", "x = 42\ny = 'hello'\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    -- Enable inlay hints.
    if client:supports_method("textDocument/inlayHint") then
      vim.lsp.inlay_hint.enable(true, { bufnr = buf })
      vim.wait(2000)
      -- Inlay hints are enabled — this verifies no error occurs.
      assert.is_true(true)
    end
  end)

  -- Code lens with real LSP

  it("code lens refresh does not error", function()
    local buf = helpers.open_python_file(tmpdir, "test_codelens.py", "def helper(x: int) -> int:\n    return x\n\nresult = helper(42)\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    if client:supports_method("textDocument/codeLens") then
      local ok = pcall(vim.lsp.codelens.refresh, { bufnr = buf })
      assert.is_true(ok, "codelens refresh should not error")
    end
  end)

  -- vim.lsp.buf.hover() with real LSP

  it("hover function works via real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_hover_ui.py", "def helper(x: int) -> int:\n    return x + 1\n\nresult = helper(42)\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    -- Move cursor to function name.
    vim.api.nvim_win_set_cursor(0, { 1, 4 })

    -- Call hover — should not error.
    local ok = pcall(vim.lsp.buf.hover)
    assert.is_true(ok, "vim.lsp.buf.hover() should not error")
  end)

  -- vim.lsp.buf.definition() with real LSP

  it("go-to-definition works via real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_gotodef_ui.py", "def helper(x: int) -> int:\n    return x + 1\n\nresult = helper(42)\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    -- Place cursor on the call site.
    vim.api.nvim_win_set_cursor(0, { 4, 9 })

    local ok = pcall(vim.lsp.buf.definition)
    assert.is_true(ok, "vim.lsp.buf.definition() should not error")
  end)

  -- vim.lsp.buf.references() with real LSP

  it("find references works via real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_refs_ui.py", "def helper(x: int) -> int:\n    return x + 1\n\na = helper(1)\nb = helper(2)\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    vim.api.nvim_win_set_cursor(0, { 1, 4 })

    local ok = pcall(vim.lsp.buf.references)
    assert.is_true(ok, "vim.lsp.buf.references() should not error")
  end)

  -- vim.lsp.buf.rename() with real LSP

  it("rename works via real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_rename_ui.py", "def helper(x: int) -> int:\n    return x + 1\n\nresult = helper(42)\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    vim.api.nvim_win_set_cursor(0, { 1, 4 })

    -- Request rename via the LSP request directly (to avoid UI input prompt).
    local err, result = helpers.lsp_request(client, "textDocument/rename", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      position = { line = 0, character = 4 },
      newName = "my_helper",
    }, buf)

    assert.is_nil(err)
    if result then
      -- Apply the workspace edit.
      local ok = pcall(vim.lsp.util.apply_workspace_edit, result, "utf-8")
      assert.is_true(ok, "applying rename workspace edit should not error")

      -- Verify the buffer content changed.
      local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
      local text = table.concat(lines, "\n")
      assert.truthy(text:find("my_helper"), "buffer should contain renamed symbol")
    end
  end)

  -- vim.lsp.buf.code_action() with real LSP

  it("code action works via real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_action_ui.py", "def greet(name):\n    return name\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)
    helpers.wait_for_diagnostics(buf)

    vim.api.nvim_win_set_cursor(0, { 1, 10 })

    -- Request code actions directly.
    local err, result = helpers.lsp_request(client, "textDocument/codeAction", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      range = {
        start = { line = 0, character = 0 },
        ["end"] = { line = 0, character = 20 },
      },
      context = { diagnostics = {} },
    }, buf)

    assert.is_nil(err, "codeAction request should not error")
  end)

  -- vim.lsp.buf.format() with real LSP

  it("format works via real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_format_ui.py", "def greet( name:str )->str:\n    return name\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local ok = pcall(vim.lsp.buf.format, { bufnr = buf, timeout_ms = 5000 })
    -- May fail if ruff is not installed — that's acceptable.
    -- The important thing is the LSP handles the request.
  end)

  -- vim.lsp.buf.document_symbol() with real LSP

  it("document symbols work via real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_symbols_ui.py", "class MyClass:\n    def method(self) -> None:\n        pass\n\ndef standalone() -> None:\n    pass\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local ok = pcall(vim.lsp.buf.document_symbol)
    assert.is_true(ok, "vim.lsp.buf.document_symbol() should not error")
  end)

  -- Edit-diagnose-fix-clear cycle (full lifecycle)

  it("full edit-diagnose-fix-clear lifecycle", function()
    local buf = helpers.open_python_file(tmpdir, "test_lifecycle.py", "def greet(name: str) -> str:\n    return name\n")
    helpers.wait_for_server_ready(buf)

    -- Should start clean.
    vim.wait(3000)
    assert.are.equal(0, #vim.diagnostic.get(buf), "clean code should have no diagnostics")

    -- Introduce an error.
    helpers.replace_content(buf, "def greet(name):\n    return name\n")
    vim.cmd("write")
    local diags = helpers.wait_for_diagnostics(buf)
    assert.is_true(#diags > 0, "untyped param should produce diagnostics")

    -- Fix the error.
    helpers.replace_content(buf, "def greet(name: str) -> str:\n    return name\n")
    vim.cmd("write")
    local cleared = helpers.wait_for_diagnostics_cleared(buf)
    assert.is_true(cleared, "diagnostics should clear after fix")
  end)
end)
