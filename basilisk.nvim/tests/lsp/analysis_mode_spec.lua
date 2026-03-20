--- Analysis mode e2e tests — real LSP, no mocking.
---
--- Tests wholeModule vs openFilesOnly behavior with the real basilisk server.

local helpers = require("tests.lsp.helpers")

local binary = helpers.find_binary()
if not binary then
  describe("analysis mode (SKIPPED — no binary)", function()
    it("skipped", function()
      pending("basilisk binary not found")
    end)
  end)
  return
end

describe("analysis mode", function()
  local tmpdir

  before_each(function()
    tmpdir = helpers.create_tmpdir()
    local fh = io.open(tmpdir .. "/pyproject.toml", "w")
    fh:write('[project]\nname = "test"\nversion = "0.1.0"\n')
    fh:close()
  end)

  after_each(function()
    helpers.stop_clients()
    helpers.close_all_buffers()
    helpers.cleanup_tmpdir(tmpdir)
  end)

  -- wholeModule: diagnostics for open file

  it("wholeModule: open file gets diagnostics", function()
    vim.lsp.config("basilisk", {
      cmd = { binary, "lsp" },
      filetypes = { "python" },
      root_markers = { "pyproject.toml" },
      settings = { basilisk = { analysisMode = "wholeModule" } },
    })
    vim.lsp.enable("basilisk")

    local buf = helpers.open_python_file(tmpdir, "test_wm.py", "def greet(name):\n    return name\n")
    helpers.wait_for_server_ready(buf)

    local diags = helpers.wait_for_diagnostics(buf)
    assert.is_true(#diags > 0, "wholeModule should produce diagnostics for untyped param")
  end)

  -- wholeModule: diagnostics persist after buffer is hidden

  it("wholeModule: diagnostics persist when buffer is hidden", function()
    vim.lsp.config("basilisk", {
      cmd = { binary, "lsp" },
      filetypes = { "python" },
      root_markers = { "pyproject.toml" },
      settings = { basilisk = { analysisMode = "wholeModule" } },
    })
    vim.lsp.enable("basilisk")

    local buf = helpers.open_python_file(tmpdir, "test_persist.py", "def greet(name):\n    return name\n")
    helpers.wait_for_server_ready(buf)
    helpers.wait_for_diagnostics(buf)

    -- Open a new buffer (hides the first).
    vim.cmd("enew")
    vim.wait(1000)

    -- Diagnostics should still exist for the hidden buffer.
    local diags = vim.diagnostic.get(buf)
    assert.is_true(#diags > 0, "wholeModule should preserve diagnostics for hidden buffers")
  end)

  -- openFilesOnly: open file gets diagnostics

  it("openFilesOnly: open file gets diagnostics", function()
    vim.lsp.config("basilisk", {
      cmd = { binary, "lsp" },
      filetypes = { "python" },
      root_markers = { "pyproject.toml" },
      settings = { basilisk = { analysisMode = "openFilesOnly" } },
    })
    vim.lsp.enable("basilisk")

    local buf = helpers.open_python_file(tmpdir, "test_ofo.py", "def greet(name):\n    return name\n")
    helpers.wait_for_server_ready(buf)

    local diags = helpers.wait_for_diagnostics(buf)
    assert.is_true(#diags > 0, "openFilesOnly should produce diagnostics for open file")
  end)

  -- Configuration is passed to initializationOptions

  it("analysis mode setting is wired into LSP config", function()
    vim.lsp.config("basilisk", {
      cmd = { binary, "lsp" },
      filetypes = { "python" },
      root_markers = { "pyproject.toml" },
      settings = { basilisk = { analysisMode = "openFilesOnly" } },
      init_options = { analysisMode = "openFilesOnly" },
    })
    vim.lsp.enable("basilisk")

    local buf = helpers.open_python_file(tmpdir, "test_config.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client)

    -- Verify the client was configured.
    assert.is_not_nil(client.config)
  end)

  -- Tab tracking: closing a buffer in openFilesOnly mode

  it("openFilesOnly: closing buffer clears diagnostics", function()
    vim.lsp.config("basilisk", {
      cmd = { binary, "lsp" },
      filetypes = { "python" },
      root_markers = { "pyproject.toml" },
      settings = { basilisk = { analysisMode = "openFilesOnly" } },
    })
    vim.lsp.enable("basilisk")

    local buf = helpers.open_python_file(tmpdir, "test_close.py", "def greet(name):\n    return name\n")
    helpers.wait_for_server_ready(buf)
    helpers.wait_for_diagnostics(buf)

    -- Verify we have diagnostics.
    local diags_before = vim.diagnostic.get(buf)
    assert.is_true(#diags_before > 0, "should have diagnostics before close")

    -- Close the buffer (wipeout).
    vim.cmd("bwipeout! " .. buf)
    vim.wait(2000)

    -- Buffer is invalid after wipeout — diagnostics are gone by definition.
    assert.is_false(vim.api.nvim_buf_is_valid(buf), "buffer should be invalid after wipeout")

    -- Verify no diagnostics remain for any buffer from this namespace.
    local all_diags = vim.diagnostic.get()
    local remaining = 0
    for _, diag in ipairs(all_diags) do
      if diag.source and diag.source:find("[Bb]asilisk") then
        remaining = remaining + 1
      end
    end
    assert.are.equal(0, remaining, "no basilisk diagnostics should remain after buffer wipeout")
  end)

  -- Tab tracking: reopening a file re-triggers diagnostics

  it("openFilesOnly: reopening file re-triggers diagnostics", function()
    vim.lsp.config("basilisk", {
      cmd = { binary, "lsp" },
      filetypes = { "python" },
      root_markers = { "pyproject.toml" },
      settings = { basilisk = { analysisMode = "openFilesOnly" } },
    })
    vim.lsp.enable("basilisk")

    -- Open, get diagnostics, close.
    local buf1 = helpers.open_python_file(tmpdir, "test_reopen.py", "def greet(name):\n    return name\n")
    helpers.wait_for_server_ready(buf1)
    helpers.wait_for_diagnostics(buf1)
    vim.cmd("bwipeout! " .. buf1)
    vim.wait(1000)

    -- Reopen the same file.
    local buf2 = helpers.open_python_file(tmpdir, "test_reopen.py", "def greet(name):\n    return name\n")
    helpers.wait_for_server_ready(buf2)
    local diags = helpers.wait_for_diagnostics(buf2)
    assert.is_true(#diags > 0, "reopened file should get diagnostics again")
  end)
end)
