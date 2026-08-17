--- Real LSP rename tests for basilisk.nvim.
---
--- These tests start the REAL basilisk LSP server and verify that
--- scope-aware rename works correctly through Neovim's LSP client.
--- NO MOCKING. Uses the actual basilisk binary.

local helpers = require("tests.lsp.helpers")

describe("LSP rename", function()
  local tmpdir
  local binary

  before_each(function()
    binary = helpers.find_binary()
    if not binary then
      pending("basilisk binary not found — skipping LSP rename tests")
      return
    end
    tmpdir = helpers.create_tmpdir()

    -- Write a pyproject.toml so basilisk finds a project root.
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
    if tmpdir then
      helpers.cleanup_tmpdir(tmpdir)
    end
  end)

  it("renames a function across definition and call sites", function()
    if not binary then
      return
    end

    local source = table.concat({
      "def helper(x: int) -> int:",
      "    return x + 1",
      "",
      "a: int = helper(1)",
      "b: int = helper(2)",
      "",
    }, "\n")

    local buf, uri = helpers.open_python_file(tmpdir, "rename_basic.py", source)
    assert.truthy(helpers.wait_for_server_ready(buf))

    local client = helpers.wait_for_client(buf)
    assert.truthy(client, "LSP client must attach")

    -- Request rename at the function definition (line 0, char 4).
    local err, result = helpers.lsp_request(client, "textDocument/rename", {
      textDocument = { uri = uri },
      position = { line = 0, character = 4 },
      newName = "assist",
    }, buf)

    assert.is_nil(err, "rename should not return an error")
    assert.truthy(result, "rename should return a workspace edit")

    -- Server may return changes or documentChanges.
    local file_edits = nil
    if result.changes then
      file_edits = result.changes[uri]
    elseif result.documentChanges then
      for _, change in ipairs(result.documentChanges) do
        if change.textDocument and change.textDocument.uri == uri then
          file_edits = change.edits
        end
      end
    end

    assert.truthy(file_edits, "should have edits for the file")
    assert(#file_edits >= 3, "expected at least 3 edits (1 def + 2 calls), got " .. #file_edits)

    for _, edit in ipairs(file_edits) do
      assert.are.equal("assist", edit.newText)
    end
  end)

  it("scope-aware: local rename does not affect module-level", function()
    if not binary then
      return
    end

    local source = table.concat({
      "x: int = 1",
      "",
      "def foo() -> int:",
      "    x: int = 2",
      "    return x",
      "",
      "y: int = x",
      "",
    }, "\n")

    local buf, uri = helpers.open_python_file(tmpdir, "rename_scope.py", source)
    assert.truthy(helpers.wait_for_server_ready(buf))

    local client = helpers.wait_for_client(buf)
    assert.truthy(client)

    -- Rename `x` inside the function (line 3, char 4).
    local err, result = helpers.lsp_request(client, "textDocument/rename", {
      textDocument = { uri = uri },
      position = { line = 3, character = 4 },
      newName = "local_x",
    }, buf)

    assert.is_nil(err)
    assert.truthy(result)
    assert.truthy(result.changes)

    -- Extract edits (server may use changes or documentChanges).
    local edits = nil
    if result.changes then
      edits = result.changes[uri]
    elseif result.documentChanges then
      for _, change in ipairs(result.documentChanges) do
        if change.textDocument and change.textDocument.uri == uri then
          edits = change.edits
        end
      end
    end
    assert.truthy(edits, "should have edits")

    -- All edits must be within the function body (lines 3-4), NOT line 0 or 6.
    for _, edit in ipairs(edits) do
      local line = edit.range.start.line
      assert(
        line >= 3 and line <= 4,
        "edit should only touch lines 3-4, but found edit on line " .. line
      )
      assert.are.equal("local_x", edit.newText)
    end
  end)

  it("scope-aware: module rename skips shadowed local", function()
    if not binary then
      return
    end

    local source = table.concat({
      "x: int = 1",
      "",
      "def foo() -> int:",
      "    x: int = 2",
      "    return x",
      "",
      "y: int = x",
      "",
    }, "\n")

    local buf, uri = helpers.open_python_file(tmpdir, "rename_module_scope.py", source)
    assert.truthy(helpers.wait_for_server_ready(buf))

    local client = helpers.wait_for_client(buf)
    assert.truthy(client)

    -- Rename `x` at module level (line 0, char 0).
    local err, result = helpers.lsp_request(client, "textDocument/rename", {
      textDocument = { uri = uri },
      position = { line = 0, character = 0 },
      newName = "global_x",
    }, buf)

    assert.is_nil(err)
    assert.truthy(result)

    local edits = nil
    if result.changes then
      edits = result.changes[uri]
    elseif result.documentChanges then
      for _, change in ipairs(result.documentChanges) do
        if change.textDocument and change.textDocument.uri == uri then
          edits = change.edits
        end
      end
    end
    assert.truthy(edits)

    -- Should only touch line 0 and line 6, NOT lines 3-4 (shadowed in function).
    for _, edit in ipairs(edits) do
      local line = edit.range.start.line
      assert(
        line == 0 or line == 6,
        "edit should only touch lines 0 and 6, but found edit on line " .. line
      )
      assert.are.equal("global_x", edit.newText)
    end
  end)

  it("rejects rename to Python keyword", function()
    if not binary then
      return
    end

    local source = "x: int = 1\n"

    local buf, uri = helpers.open_python_file(tmpdir, "rename_keyword.py", source)
    assert.truthy(helpers.wait_for_server_ready(buf))

    local client = helpers.wait_for_client(buf)
    assert.truthy(client)

    -- Rename `x` to `class` (keyword).
    local err, result = helpers.lsp_request(client, "textDocument/rename", {
      textDocument = { uri = uri },
      position = { line = 0, character = 0 },
      newName = "class",
    }, buf)

    -- Should return null/nil result (rejected).
    assert.is_nil(result, "rename to keyword 'class' should return nil result")
  end)

  it("nested function: outer rename skips inner shadow", function()
    if not binary then
      return
    end

    local source = table.concat({
      "def outer() -> int:",
      "    x: int = 1",
      "    def inner() -> int:",
      "        x: int = 2",
      "        return x",
      "    return x",
      "",
    }, "\n")

    local buf, uri = helpers.open_python_file(tmpdir, "rename_nested.py", source)
    assert.truthy(helpers.wait_for_server_ready(buf))

    local client = helpers.wait_for_client(buf)
    assert.truthy(client)

    -- Rename `x` in outer function (line 1, char 4).
    local err, result = helpers.lsp_request(client, "textDocument/rename", {
      textDocument = { uri = uri },
      position = { line = 1, character = 4 },
      newName = "outer_x",
    }, buf)

    assert.is_nil(err)
    assert.truthy(result)

    local edits = nil
    if result.changes then
      edits = result.changes[uri]
    elseif result.documentChanges then
      for _, change in ipairs(result.documentChanges) do
        if change.textDocument and change.textDocument.uri == uri then
          edits = change.edits
        end
      end
    end
    assert.truthy(edits)

    -- Should rename on lines 1 and 5 (outer), NOT lines 3-4 (inner shadow).
    for _, edit in ipairs(edits) do
      local line = edit.range.start.line
      assert(
        line == 1 or line == 5,
        "edit should only touch lines 1 and 5, but found edit on line " .. line
      )
      assert.are.equal("outer_x", edit.newText)
    end
  end)

  it("prepareRename returns range for valid position", function()
    if not binary then
      return
    end

    local source = "def greet(name: str) -> str:\n    return name\n"

    local buf, uri = helpers.open_python_file(tmpdir, "prepare_rename.py", source)
    assert.truthy(helpers.wait_for_server_ready(buf))

    local client = helpers.wait_for_client(buf)
    assert.truthy(client)

    local err, result = helpers.lsp_request(client, "textDocument/prepareRename", {
      textDocument = { uri = uri },
      position = { line = 0, character = 4 },
    }, buf)

    assert.is_nil(err)
    assert.truthy(result, "prepareRename should return a result for a function name")
  end)
end)
