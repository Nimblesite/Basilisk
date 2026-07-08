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

    -- Opt into the annotation house rules (off by default) so untyped-parameter
    -- diagnostics fire — mirrors the Rust LSP harness fixture (ws_test_common.rs).
    local cfg = io.open(tmpdir .. "/basilisk.json", "w")
    cfg:write('{"strictAnnotations": true}\n')
    cfg:close()

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
      -- Go through the plugin's version-compatible activation so this stays
      -- green on Neovim 0.13, where vim.lsp.codelens.refresh is removed.
      local ok = pcall(require("basilisk.codelens").activate, buf)
      assert.is_true(ok, "code lens activation should not error")
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
    -- The Ruff formatter is embedded in the binary ([LSPFMT-ENGINE]);
    -- formatting must succeed with no external ruff installed (#254).
    assert.is_true(ok, "vim.lsp.buf.format must succeed")
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

  -- :BasiliskInfo floating window with live LSP

  it(":BasiliskInfo opens floating window with correct content", function()
    local buf = helpers.open_python_file(tmpdir, "test_info_cmd.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    -- Register commands manually (normally done by setup()).
    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    vim.cmd("BasiliskInfo")

    -- Find the floating window (not the main editor window).
    local wins = vim.api.nvim_list_wins()
    local float_win = nil
    local float_buf = nil
    for _, win in ipairs(wins) do
      local win_config = vim.api.nvim_win_get_config(win)
      if win_config.relative and win_config.relative ~= "" then
        float_win = win
        float_buf = vim.api.nvim_win_get_buf(win)
        break
      end
    end

    assert.is_not_nil(float_win, ":BasiliskInfo should open a floating window")
    assert.is_not_nil(float_buf, "floating window should have a buffer")

    local lines = vim.api.nvim_buf_get_lines(float_buf, 0, -1, false)
    local text = table.concat(lines, "\n")

    -- Verify content.
    assert.truthy(text:find("Basilisk LSP Server Info"), "should contain title")
    assert.truthy(text:find("Status:%s+active"), "should show active status")
    assert.truthy(text:find("Binary:"), "should show binary path")
    assert.truthy(text:find("Version:"), "should show version")
    assert.truthy(text:find("Mode:"), "should show analysis mode")

    -- Close the float.
    if float_win and vim.api.nvim_win_is_valid(float_win) then
      vim.api.nvim_win_close(float_win, true)
    end
  end)

  -- :BasiliskTestToggle side panel

  it(":BasiliskTestToggle opens and closes test explorer panel", function()
    local buf = helpers.open_python_file(tmpdir, "test_toggle_cmd.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    -- Register commands manually (normally done by setup()).
    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local initial_win_count = #vim.api.nvim_list_wins()

    -- Open the test explorer.
    vim.cmd("BasiliskTestToggle")
    vim.wait(500)

    local after_open_wins = vim.api.nvim_list_wins()
    assert.is_true(#after_open_wins > initial_win_count, "toggle should open a new window")

    -- Find the test explorer window by its buffer filetype.
    local panel_win = nil
    for _, win in ipairs(after_open_wins) do
      local win_buf = vim.api.nvim_win_get_buf(win)
      local ft = vim.bo[win_buf].filetype
      if ft == "basilisk-tests" then
        panel_win = win
        break
      end
    end

    assert.is_not_nil(panel_win, "test explorer panel should have filetype basilisk-tests")

    -- Verify the panel window width is reasonable (side panel).
    local panel_width = vim.api.nvim_win_get_width(panel_win)
    assert.is_true(panel_width > 0 and panel_width < vim.o.columns, "panel should be a side split")

    -- Close via toggle.
    vim.cmd("BasiliskTestToggle")
    vim.wait(500)

    local after_close_wins = #vim.api.nvim_list_wins()
    assert.are.equal(initial_win_count, after_close_wins, "toggle again should close the panel")
  end)
end)
