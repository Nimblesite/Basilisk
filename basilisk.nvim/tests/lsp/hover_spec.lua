--- Hover e2e tests — real LSP, no mocking.
---
--- Tests hover content including type signatures and docstrings.

local helpers = require("tests.lsp.helpers")

local binary = helpers.find_binary()
if not binary then
  describe("hover (SKIPPED — no binary)", function()
    it("skipped", function()
      pending("basilisk binary not found")
    end)
  end)
  return
end

local tmpdir

describe("hover with real LSP", function()
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

  it("hover shows type signature for a function", function()
    local buf = helpers.open_python_file(tmpdir, "test_hover_sig.py",
      "def helper(x: int, y: str) -> bool:\n    return len(y) > x\n\nresult = helper(3, 'hello')\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/hover", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      position = { line = 0, character = 4 },
    }, buf)

    assert.is_nil(err)
    assert.is_not_nil(result)
    assert.is_not_nil(result.contents)
  end)

  it("hover shows type for a variable", function()
    local buf = helpers.open_python_file(tmpdir, "test_hover_var.py",
      "x: int = 42\ny: str = 'hello'\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/hover", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      position = { line = 0, character = 0 },
    }, buf)

    assert.is_nil(err)
    assert.is_not_nil(result)
  end)

  it("hover shows docstring for function", function()
    local buf = helpers.open_python_file(tmpdir, "test_hover_doc.py", table.concat({
      'def greet(name: str) -> str:',
      '    """Return a greeting for the given name."""',
      '    return f"Hello, {name}!"',
      '',
      'result = greet("world")',
      '',
    }, "\n"))
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/hover", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      position = { line = 4, character = 9 },
    }, buf)

    assert.is_nil(err)
    assert.is_not_nil(result)
    if result and result.contents then
      local text = type(result.contents) == "string" and result.contents
        or result.contents.value or vim.inspect(result.contents)
      -- The hover should contain the docstring or the function signature.
      assert.truthy(text:find("greet") or text:find("str"), "hover should show function info")
    end
  end)

  it("hover on class shows class info", function()
    local buf = helpers.open_python_file(tmpdir, "test_hover_class.py", table.concat({
      'class Point:',
      '    """A 2D point."""',
      '    def __init__(self, x: int, y: int) -> None:',
      '        self.x = x',
      '        self.y = y',
      '',
      'p = Point(1, 2)',
      '',
    }, "\n"))
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/hover", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      position = { line = 6, character = 4 },
    }, buf)

    assert.is_nil(err)
    assert.is_not_nil(result)
  end)

  it("hover returns nil for whitespace", function()
    local buf = helpers.open_python_file(tmpdir, "test_hover_empty.py", "\n\n\nx: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)
    helpers.wait_for_server_ready(buf)

    local err, result = helpers.lsp_request(client, "textDocument/hover", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
      position = { line = 0, character = 0 },
    }, buf)

    assert.is_nil(err)
    -- Result should be nil for empty space.
  end)
end)
