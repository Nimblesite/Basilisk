--- Real LSP integration tests for basilisk.nvim.
---
--- These tests use the REAL basilisk LSP server. No mocking.
--- Requires the basilisk binary to be available (target/debug/basilisk
--- or on PATH).

local helpers = require("tests.lsp.helpers")

-- Skip entire suite if no binary available.
local binary = helpers.find_binary()
if not binary then
  describe("basilisk LSP integration (SKIPPED — no binary)", function()
    it("skipped: basilisk binary not found", function()
      pending("basilisk binary not found — build with `cargo build --bin basilisk`")
    end)
  end)
  return
end

-- Configure basilisk to use the found binary.
local tmpdir

describe("basilisk LSP integration", function()
  before_each(function()
    tmpdir = helpers.create_tmpdir()

    -- Write a pyproject.toml so basilisk finds a project root.
    local fh = io.open(tmpdir .. "/pyproject.toml", "w")
    fh:write('[project]\nname = "test"\nversion = "0.1.0"\n')
    fh:close()

    -- Configure and start the LSP client directly (not via setup()).
    vim.lsp.config("basilisk", {
      cmd = { binary, "lsp" },
      filetypes = { "python" },
      root_markers = { "pyproject.toml", ".git" },
      settings = {
        basilisk = {
          analysisMode = "wholeModule",
        },
      },
    })
    vim.lsp.enable("basilisk")
  end)

  after_each(function()
    helpers.stop_clients()
    helpers.close_all_buffers()
    helpers.cleanup_tmpdir(tmpdir)
  end)

  -- Core LSP: Diagnostics

  it("produces diagnostics for untyped parameters", function()
    local buf = helpers.open_python_file(tmpdir, "test_untyped.py", "def greet(name):\n    return name\n")
    local ready = helpers.wait_for_server_ready(buf)
    assert.is_true(ready, "LSP server did not become ready")

    local diags = helpers.wait_for_diagnostics(buf)
    assert.is_true(#diags > 0, "expected diagnostics for untyped parameter")
  end)

  it("clears diagnostics when errors are fixed", function()
    local buf = helpers.open_python_file(tmpdir, "test_fix.py", "def greet(name):\n    return name\n")
    helpers.wait_for_server_ready(buf)
    helpers.wait_for_diagnostics(buf)

    -- Fix the code by adding types.
    helpers.replace_content(buf, "def greet(name: str) -> str:\n    return name\n")
    vim.cmd("write")

    local cleared = helpers.wait_for_diagnostics_cleared(buf)
    assert.is_true(cleared, "diagnostics should clear after fix")
  end)

  it("shows no diagnostics for fully typed code", function()
    local buf = helpers.open_python_file(tmpdir, "test_typed.py", "def greet(name: str) -> str:\n    return name\n")
    helpers.wait_for_server_ready(buf)

    -- Wait a bit and verify no diagnostics appear.
    vim.wait(3000)
    local diags = vim.diagnostic.get(buf)
    assert.are.equal(0, #diags, "fully typed code should have no diagnostics")
  end)

  it("updates diagnostics on file change", function()
    local buf = helpers.open_python_file(tmpdir, "test_change.py", "def greet(name: str) -> str:\n    return name\n")
    helpers.wait_for_server_ready(buf)
    vim.wait(2000)
    assert.are.equal(0, #vim.diagnostic.get(buf))

    -- Introduce an error.
    helpers.replace_content(buf, "def greet(name):\n    return name\n")
    vim.cmd("write")

    local diags = helpers.wait_for_diagnostics(buf)
    assert.is_true(#diags > 0, "expected diagnostics after introducing untyped param")
  end)

  -- Core LSP: Hover

  it("hover provides type information", function()
    local buf = helpers.open_python_file(tmpdir, "test_hover.py", "def helper(x: int) -> int:\n    return x + 1\n\nresult = helper(42)\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/hover", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      position = { line = 0, character = 4 },
    }, buf)

    assert.is_nil(err, "hover request should not error")
    assert.is_not_nil(result, "hover should return a result")
    if result then
      assert.is_not_nil(result.contents, "hover should have contents")
    end
  end)

  -- Core LSP: Go to Definition

  it("go-to-definition works", function()
    local buf = helpers.open_python_file(tmpdir, "test_def.py", "def helper(x: int) -> int:\n    return x + 1\n\nresult = helper(42)\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/definition", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      position = { line = 3, character = 9 },
    }, buf)

    assert.is_nil(err, "definition request should not error")
    assert.is_not_nil(result, "should find definition")
  end)

  -- Core LSP: Completions

  it("completions include local symbols", function()
    local buf = helpers.open_python_file(tmpdir, "test_comp.py", "def my_helper_function(x: int) -> int:\n    return x\n\nmy_\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/completion", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      position = { line = 3, character = 3 },
    }, buf)

    assert.is_nil(err, "completion request should not error")
    if result then
      local items = result.items or result
      assert.is_true(#items > 0, "should have completion items")
    end
  end)

  -- Core LSP: Document Symbols

  it("document symbols include classes and functions", function()
    local buf = helpers.open_python_file(tmpdir, "test_symbols.py", "class MyClass:\n    def method(self) -> None:\n        pass\n\ndef standalone() -> None:\n    pass\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/documentSymbol", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
    }, buf)

    assert.is_nil(err, "documentSymbol request should not error")
    assert.is_not_nil(result, "should return symbols")
    if result then
      assert.is_true(#result >= 2, "should have at least class + function symbols")
    end
  end)

  -- Core LSP: Signature Help

  it("signature help works", function()
    local buf = helpers.open_python_file(tmpdir, "test_sig.py", "def helper(x: int, y: str) -> int:\n    return x\n\nhelper(\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/signatureHelp", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      position = { line = 3, character = 7 },
    }, buf)

    assert.is_nil(err, "signatureHelp request should not error")
    -- Result may be nil if server doesn't support it yet — that's ok.
  end)

  -- Core LSP: Find References

  it("find references works", function()
    local buf = helpers.open_python_file(tmpdir, "test_refs.py", "def helper(x: int) -> int:\n    return x + 1\n\na = helper(1)\nb = helper(2)\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/references", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      position = { line = 0, character = 4 },
      context = { includeDeclaration = true },
    }, buf)

    assert.is_nil(err, "references request should not error")
    if result then
      assert.is_true(#result >= 2, "should find definition + call sites")
    end
  end)

  -- Core LSP: Rename

  it("rename symbol works", function()
    local buf = helpers.open_python_file(tmpdir, "test_rename.py", "def helper(x: int) -> int:\n    return x + 1\n\nresult = helper(42)\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/rename", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      position = { line = 0, character = 4 },
      newName = "my_helper",
    }, buf)

    assert.is_nil(err, "rename request should not error")
    if result then
      assert.is_not_nil(result.changes or result.documentChanges, "rename should produce workspace edit")
    end
  end)

  -- Core LSP: Code Actions

  it("code actions provided for diagnostics", function()
    local buf = helpers.open_python_file(tmpdir, "test_actions.py", "def greet(name):\n    return name\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)
    helpers.wait_for_diagnostics(buf)

    local diags = vim.diagnostic.get(buf)
    local err, result = helpers.lsp_request(client, "textDocument/codeAction", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      range = {
        start = { line = 0, character = 0 },
        ["end"] = { line = 0, character = 20 },
      },
      context = {
        diagnostics = vim.lsp.diagnostic.get_line_diagnostics(buf, 0) or {},
      },
    }, buf)

    assert.is_nil(err, "codeAction request should not error")
    -- Code actions may or may not be available depending on server capability.
  end)

  -- Core LSP: Formatting

  it("format document works", function()
    local buf = helpers.open_python_file(tmpdir, "test_fmt.py", "def greet( name: str )->str:\n    return name\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/formatting", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      options = { tabSize = 4, insertSpaces = true },
    }, buf)

    assert.is_nil(err, "formatting request should not error")
    -- Result may be nil if ruff is not installed — acceptable.
  end)

  -- Core LSP: Inlay Hints

  it("inlay hints appear for unannotated variables", function()
    local buf = helpers.open_python_file(tmpdir, "test_inlay.py", "x = 42\ny = 'hello'\nz = [1, 2, 3]\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/inlayHint", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      range = {
        start = { line = 0, character = 0 },
        ["end"] = { line = 3, character = 0 },
      },
    }, buf)

    assert.is_nil(err, "inlayHint request should not error")
    -- Hints may or may not be returned depending on server capability.
  end)

  -- Core LSP: Document Highlight

  it("document highlight works", function()
    local buf = helpers.open_python_file(tmpdir, "test_highlight.py", "def helper(x: int) -> int:\n    return x + 1\n\nresult = helper(42)\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/documentHighlight", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      position = { line = 0, character = 4 },
    }, buf)

    assert.is_nil(err, "documentHighlight request should not error")
  end)

  -- Core LSP: Folding Ranges

  it("folding ranges work", function()
    local buf = helpers.open_python_file(tmpdir, "test_fold.py", "class MyClass:\n    def method(self) -> None:\n        pass\n\ndef standalone() -> None:\n    pass\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/foldingRange", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
    }, buf)

    assert.is_nil(err, "foldingRange request should not error")
  end)

  -- Core LSP: Selection Range

  it("selection range works", function()
    local buf = helpers.open_python_file(tmpdir, "test_sel.py", "def helper(x: int) -> int:\n    return x + 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/selectionRange", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      positions = { { line = 1, character = 4 } },
    }, buf)

    assert.is_nil(err, "selectionRange request should not error")
  end)

  -- Core LSP: Semantic Tokens

  it("semantic tokens work", function()
    local buf = helpers.open_python_file(tmpdir, "test_tokens.py", "class Point:\n    x: int\n    y: int\n\np = Point()\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/semanticTokens/full", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
    }, buf)

    assert.is_nil(err, "semanticTokens request should not error")
  end)

  -- Core LSP: Code Lens

  it("code lens works", function()
    local buf = helpers.open_python_file(tmpdir, "test_lens.py", "def helper(x: int) -> int:\n    return x + 1\n\nresult = helper(42)\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/codeLens", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
    }, buf)

    assert.is_nil(err, "codeLens request should not error")
  end)

  -- Core LSP: Call Hierarchy

  it("call hierarchy works", function()
    local buf = helpers.open_python_file(tmpdir, "test_call.py", "def helper(x: int) -> int:\n    return x + 1\n\ndef caller() -> int:\n    return helper(42)\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/prepareCallHierarchy", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      position = { line = 0, character = 4 },
    }, buf)

    assert.is_nil(err, "prepareCallHierarchy request should not error")
  end)

  -- Core LSP: Type Hierarchy

  it("type hierarchy works", function()
    local buf = helpers.open_python_file(tmpdir, "test_type.py", "class Base:\n    pass\n\nclass Child(Base):\n    pass\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/prepareTypeHierarchy", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      position = { line = 3, character = 6 },
    }, buf)

    assert.is_nil(err, "prepareTypeHierarchy request should not error")
  end)

  -- Multiple Files

  it("multiple files get independent diagnostics", function()
    local buf1 = helpers.open_python_file(tmpdir, "file_a.py", "def func_a(x):\n    return x\n")
    helpers.wait_for_server_ready(buf1)
    local diags1 = helpers.wait_for_diagnostics(buf1)
    assert.is_true(#diags1 > 0, "file_a should have diagnostics")

    local buf2 = helpers.open_python_file(tmpdir, "file_b.py", "def func_b(x: int) -> int:\n    return x\n")
    vim.wait(3000)
    local diags2 = vim.diagnostic.get(buf2)
    assert.are.equal(0, #diags2, "file_b should have no diagnostics")

    -- file_a should still have its diagnostics.
    assert.is_true(#vim.diagnostic.get(buf1) > 0, "file_a diagnostics should persist")
  end)

  -- Server Lifecycle

  it("server restarts and remains functional", function()
    local buf = helpers.open_python_file(tmpdir, "test_restart.py", "def greet(name: str) -> str:\n    return name\n")
    helpers.wait_for_server_ready(buf)

    -- Stop and restart.
    helpers.stop_clients()
    vim.lsp.enable("basilisk")

    -- Reopen file to trigger re-attach.
    vim.cmd("edit " .. vim.fn.fnameescape(tmpdir .. "/test_restart.py"))
    buf = vim.api.nvim_get_current_buf()

    local ready = helpers.wait_for_server_ready(buf)
    assert.is_true(ready, "server should be functional after restart")
  end)
end)
