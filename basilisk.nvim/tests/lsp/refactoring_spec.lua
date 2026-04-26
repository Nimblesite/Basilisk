--- Real LSP refactoring code action tests for basilisk.nvim.
---
--- These tests start the REAL basilisk LSP server and verify that
--- code actions are returned for various refactoring scenarios.
--- NO MOCKING. Uses the actual basilisk binary.

local helpers = require("tests.lsp.helpers")

describe("LSP refactoring code actions", function()
  local tmpdir
  local binary

  before_each(function()
    binary = helpers.find_binary()
    if not binary then
      pending("basilisk binary not found — skipping LSP refactoring tests")
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

  it("extract variable code action is offered", function()
    if not binary then
      return
    end

    local source = "result: int = some_func(42) + other_func(7)\n"
    local buf, uri = helpers.open_python_file(tmpdir, "extract_var.py", source)
    assert.truthy(helpers.wait_for_server_ready(buf))

    local client = helpers.wait_for_client(buf)
    assert.truthy(client)

    local err, result = helpers.lsp_request(client, "textDocument/codeAction", {
      textDocument = { uri = uri },
      range = {
        start = { line = 0, character = 14 },
        ["end"] = { line = 0, character = 27 },
      },
      context = { diagnostics = {} },
    }, buf)

    assert.is_nil(err)
    assert.truthy(result)

    local found = false
    for _, action in ipairs(result) do
      if action.title and action.title:find("Extract variable") then
        found = true
        break
      end
    end
    assert.is_true(found, "should offer Extract variable code action")
  end)

  it("inline variable code action is offered", function()
    if not binary then
      return
    end

    local source = "def f() -> None:\n    temp = calculate()\n    result = temp + 1\n"
    local buf, uri = helpers.open_python_file(tmpdir, "inline_var.py", source)
    assert.truthy(helpers.wait_for_server_ready(buf))

    local client = helpers.wait_for_client(buf)
    assert.truthy(client)

    local err, result = helpers.lsp_request(client, "textDocument/codeAction", {
      textDocument = { uri = uri },
      range = {
        start = { line = 1, character = 4 },
        ["end"] = { line = 1, character = 4 },
      },
      context = { diagnostics = {} },
    }, buf)

    assert.is_nil(err)
    assert.truthy(result)

    local found = false
    for _, action in ipairs(result) do
      if action.title and action.title:find("Inline variable") then
        found = true
        break
      end
    end
    assert.is_true(found, "should offer Inline variable code action")
  end)

  it("Union conversion code action is offered", function()
    if not binary then
      return
    end

    local source = "from typing import Union\nx: Union[int, str] = 1\n"
    local buf, uri = helpers.open_python_file(tmpdir, "union_convert.py", source)
    assert.truthy(helpers.wait_for_server_ready(buf))

    local client = helpers.wait_for_client(buf)
    assert.truthy(client)

    local err, result = helpers.lsp_request(client, "textDocument/codeAction", {
      textDocument = { uri = uri },
      range = {
        start = { line = 1, character = 3 },
        ["end"] = { line = 1, character = 3 },
      },
      context = { diagnostics = {} },
    }, buf)

    assert.is_nil(err)
    assert.truthy(result)

    local found = false
    for _, action in ipairs(result) do
      if action.title and action.title:find("Union") then
        found = true
        break
      end
    end
    assert.is_true(found, "should offer Union conversion code action")
  end)

  it("f-string conversion code action is offered", function()
    if not binary then
      return
    end

    local source = 'name: str = "world"\nx: str = f"hello {name}"\n'
    local buf, uri = helpers.open_python_file(tmpdir, "fstring_convert.py", source)
    assert.truthy(helpers.wait_for_server_ready(buf))

    local client = helpers.wait_for_client(buf)
    assert.truthy(client)

    local err, result = helpers.lsp_request(client, "textDocument/codeAction", {
      textDocument = { uri = uri },
      range = {
        start = { line = 1, character = 9 },
        ["end"] = { line = 1, character = 9 },
      },
      context = { diagnostics = {} },
    }, buf)

    assert.is_nil(err)
    assert.truthy(result)

    local found = false
    for _, action in ipairs(result) do
      if action.title and action.title:find(".format%(%)") then
        found = true
        break
      end
    end
    assert.is_true(found, "should offer .format() conversion code action")
  end)

  it("move symbol code action is offered for class", function()
    if not binary then
      return
    end

    local source = "import os\n\nclass MyWidget:\n    pass\n"
    local buf, uri = helpers.open_python_file(tmpdir, "move_symbol.py", source)
    assert.truthy(helpers.wait_for_server_ready(buf))

    local client = helpers.wait_for_client(buf)
    assert.truthy(client)

    local err, result = helpers.lsp_request(client, "textDocument/codeAction", {
      textDocument = { uri = uri },
      range = {
        start = { line = 2, character = 0 },
        ["end"] = { line = 2, character = 0 },
      },
      context = { diagnostics = {} },
    }, buf)

    assert.is_nil(err)
    assert.truthy(result)

    local found_move = false
    local found_new_file = false
    for _, action in ipairs(result) do
      if action.title then
        if action.title:find("Move") then
          found_move = true
        end
        if action.title:find("new file") then
          found_new_file = true
        end
      end
    end
    assert.is_true(found_move, "should offer Move code action")
    assert.is_true(found_new_file, "should offer new file code action")
  end)

  it("change signature remove parameter is offered", function()
    if not binary then
      return
    end

    local source = "def greet(name: str, greeting: str) -> str:\n    return f\"{greeting}, {name}\"\n\nresult: str = greet(\"world\", \"Hello\")\n"
    local buf, uri = helpers.open_python_file(tmpdir, "change_sig.py", source)
    assert.truthy(helpers.wait_for_server_ready(buf))

    local client = helpers.wait_for_client(buf)
    assert.truthy(client)

    local err, result = helpers.lsp_request(client, "textDocument/codeAction", {
      textDocument = { uri = uri },
      range = {
        start = { line = 0, character = 21 },
        ["end"] = { line = 0, character = 28 },
      },
      context = { diagnostics = {} },
    }, buf)

    assert.is_nil(err)
    assert.truthy(result)

    local found = false
    for _, action in ipairs(result) do
      if action.title and action.title:find("Remove parameter") then
        found = true
        break
      end
    end
    assert.is_true(found, "should offer Remove parameter code action")
  end)

  it("implement abstract methods is offered", function()
    if not binary then
      return
    end

    local source = "from abc import ABC, abstractmethod\n\nclass Base(ABC):\n    @abstractmethod\n    def do_thing(self) -> None:\n        ...\n\nclass Child(Base):\n    pass\n"
    local buf, uri = helpers.open_python_file(tmpdir, "impl_abstract.py", source)
    assert.truthy(helpers.wait_for_server_ready(buf))

    local client = helpers.wait_for_client(buf)
    assert.truthy(client)

    local err, result = helpers.lsp_request(client, "textDocument/codeAction", {
      textDocument = { uri = uri },
      range = {
        start = { line = 7, character = 6 },
        ["end"] = { line = 7, character = 6 },
      },
      context = { diagnostics = {} },
    }, buf)

    assert.is_nil(err)
    assert.truthy(result)

    local found = false
    for _, action in ipairs(result) do
      if action.title and (action.title:find("abstract") or action.title:find("Implement")) then
        found = true
        break
      end
    end
    assert.is_true(found, "should offer implement abstract methods code action")
  end)
end)
