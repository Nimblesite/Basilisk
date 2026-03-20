--- DAP integration E2E tests for basilisk.nvim.
---
--- These tests exercise REAL debug sessions by:
---   1. Starting the basilisk LSP server
---   2. Using nvim-dap to launch debugpy via the LSP
---   3. Setting breakpoints, stepping, and asserting variable values
---
--- Prerequisites:
---   - basilisk binary (cargo build -p basilisk-cli)
---   - Python 3 + debugpy (pip install debugpy)
---   - nvim-dap on the runtimepath

local lsp_helpers = require("tests.lsp.helpers")
local dap_helpers = require("tests.dap.helpers")

-- Skip the entire suite if prerequisites are missing.
local binary = lsp_helpers.find_binary()
if not binary then
  describe("basilisk DAP integration (SKIPPED — no binary)", function()
    it("skipped: basilisk binary not found", function()
      pending("basilisk binary not found — build with `cargo build --bin basilisk`")
    end)
  end)
  return
end

local dap_ok, dap = pcall(require, "dap")
if not dap_ok then
  describe("basilisk DAP integration (SKIPPED — no nvim-dap)", function()
    it("skipped: nvim-dap not found", function()
      pending("nvim-dap not found — install mfussenegger/nvim-dap")
    end)
  end)
  return
end

if not dap_helpers.is_debugpy_installed() then
  describe("basilisk DAP integration (SKIPPED — no debugpy)", function()
    it("skipped: debugpy not installed", function()
      pending("debugpy not installed — run `pip install debugpy`")
    end)
  end)
  return
end

local fixture_path = dap_helpers.fixture_path()
if not fixture_path then
  describe("basilisk DAP integration (SKIPPED — no fixture)", function()
    it("skipped: debug_stepping.py not found", function()
      pending("fixture not found — check vscode-extension/src/test/fixtures/debug_stepping.py")
    end)
  end)
  return
end

-- Suppress nvim-dap's interactive prompts in headless mode.
-- When nvim-dap calls vim.ui.select (e.g., for thread selection), auto-pick
-- the first option to avoid blocking.
vim.ui.select = function(items, opts, on_choice)
  on_choice(items[1], 1)
end

-- ── Test Suite ─────────────────────────────────────────────────────────────

local tmpdir

describe("basilisk DAP integration", function()
  before_each(function()
    tmpdir = lsp_helpers.create_tmpdir()

    -- Write a pyproject.toml so basilisk finds a project root.
    local fh = io.open(tmpdir .. "/pyproject.toml", "w")
    assert(fh, "failed to create pyproject.toml")
    fh:write('[project]\nname = "test"\nversion = "0.1.0"\n')
    fh:close()

    -- Copy the fixture into the temp directory.
    local src = io.open(fixture_path, "r")
    assert(src, "failed to read fixture")
    local content = src:read("*a")
    src:close()

    local dst = io.open(tmpdir .. "/debug_stepping.py", "w")
    assert(dst, "failed to write fixture")
    dst:write(content)
    dst:close()

    -- Configure and start the LSP.
    vim.lsp.config("basilisk", {
      cmd = { binary, "lsp" },
      filetypes = { "python" },
      root_markers = { "pyproject.toml", ".git" },
    })
    vim.lsp.enable("basilisk")

    -- Set up DAP adapter via the basilisk module.
    local basilisk_dap = require("basilisk.dap")
    basilisk_dap.setup({
      debugger = { enabled = true },
      python = "python3",
    })
  end)

  after_each(function()
    dap_helpers.cleanup_session()
    lsp_helpers.stop_clients()
    lsp_helpers.close_all_buffers()

    -- Clear breakpoints.
    dap.clear_breakpoints()

    lsp_helpers.cleanup_tmpdir(tmpdir)
  end)

  -- ── Session lifecycle ───────────────────────────────────────────────

  it("starts and stops a debug session", function()
    local filepath = tmpdir .. "/debug_stepping.py"
    vim.cmd("edit " .. vim.fn.fnameescape(filepath))
    local buf = vim.api.nvim_get_current_buf()
    local ready = lsp_helpers.wait_for_server_ready(buf)
    assert.is_true(ready, "LSP server did not become ready")

    -- Launch via nvim-dap.
    dap.run({
      type = "basilisk",
      request = "launch",
      name = "Test: debug_stepping.py",
      program = tmpdir .. "/debug_stepping.py",
      justMyCode = true,
    })

    -- Wait for the session to start.
    local session_active = dap_helpers.wait_for_session()
    assert.is_true(session_active, "debug session did not start")

    -- Terminate.
    dap.terminate()
    local terminated = dap_helpers.wait_for_terminated()
    assert.is_true(terminated, "debug session did not terminate")
  end)

  -- ── Breakpoint hitting ──────────────────────────────────────────────

  it("hits a breakpoint and stops", function()
    local filepath = tmpdir .. "/debug_stepping.py"
    vim.cmd("edit " .. vim.fn.fnameescape(filepath))
    local buf = vim.api.nvim_get_current_buf()
    lsp_helpers.wait_for_server_ready(buf)

    -- Set breakpoint on line 15 (result = w - 5 in arithmetic()).
    -- Using a line deep inside the function avoids module-level stops.
    vim.api.nvim_win_set_cursor(0, { 15, 0 })
    dap.toggle_breakpoint()

    dap.run({
      type = "basilisk",
      request = "launch",
      name = "Test: breakpoint",
      program = filepath,
      justMyCode = true,
    })

    local stopped = dap_helpers.wait_for_stopped()
    assert.is_true(stopped, "did not stop at breakpoint")

    -- Verify we stopped in the right function.
    local frames = dap_helpers.get_stack_trace()
    assert.is_true(#frames > 0, "no stack frames")
    assert.are.equal("arithmetic", frames[1].name)

    -- Verify the variables are set at this point.
    local vars = dap_helpers.get_local_variables()
    assert.are.equal("60", vars["w"])
  end)

  -- ── Stepping and variable inspection ────────────────────────────────

  it("steps through arithmetic and inspects variables", function()
    local filepath = tmpdir .. "/debug_stepping.py"
    vim.cmd("edit " .. vim.fn.fnameescape(filepath))
    local buf = vim.api.nvim_get_current_buf()
    lsp_helpers.wait_for_server_ready(buf)

    -- Breakpoint on line 11 (x = 10).
    vim.api.nvim_win_set_cursor(0, { 11, 0 })
    dap.toggle_breakpoint()

    dap.run({
      type = "basilisk",
      request = "launch",
      name = "Test: stepping",
      program = filepath,
      justMyCode = true,
    })

    local stopped = dap_helpers.wait_for_stopped()
    assert.is_true(stopped, "did not stop at breakpoint")

    -- Step to x = 10 (line 11 → 12).
    dap_helpers.step_and_wait("next")
    local vars = dap_helpers.get_local_variables()
    assert.are.equal("10", vars["x"])

    -- Step to y = 20 (line 12 → 13).
    dap_helpers.step_and_wait("next")
    vars = dap_helpers.get_local_variables()
    assert.are.equal("20", vars["y"])

    -- Step to z = x + y (line 13 → 14).
    dap_helpers.step_and_wait("next")
    vars = dap_helpers.get_local_variables()
    assert.are.equal("30", vars["z"])

    -- Step to w = z * 2 (line 14 → 15).
    dap_helpers.step_and_wait("next")
    vars = dap_helpers.get_local_variables()
    assert.are.equal("60", vars["w"])

    -- Step to result = w - 5 (line 15 → 16).
    dap_helpers.step_and_wait("next")
    vars = dap_helpers.get_local_variables()
    assert.are.equal("55", vars["result"])
  end)

  -- ── Multiple breakpoints + continue ─────────────────────────────────

  it("continues between multiple breakpoints", function()
    local filepath = tmpdir .. "/debug_stepping.py"
    vim.cmd("edit " .. vim.fn.fnameescape(filepath))
    local buf = vim.api.nvim_get_current_buf()
    lsp_helpers.wait_for_server_ready(buf)

    -- Breakpoint on arithmetic line 11 and string_ops line 21.
    vim.api.nvim_win_set_cursor(0, { 11, 0 })
    dap.toggle_breakpoint()
    vim.api.nvim_win_set_cursor(0, { 21, 0 })
    dap.toggle_breakpoint()

    dap.run({
      type = "basilisk",
      request = "launch",
      name = "Test: multiple breakpoints",
      program = filepath,
      justMyCode = true,
    })

    -- First breakpoint: arithmetic().
    local stopped = dap_helpers.wait_for_stopped()
    assert.is_true(stopped, "did not stop at first breakpoint")
    local frames = dap_helpers.get_stack_trace()
    assert.are.equal("arithmetic", frames[1].name)

    -- Continue to second breakpoint: string_ops().
    stopped = dap_helpers.continue_and_wait()
    assert.is_true(stopped, "did not stop at second breakpoint")
    frames = dap_helpers.get_stack_trace()
    assert.are.equal("string_ops", frames[1].name)
  end)

  -- ── String operations ───────────────────────────────────────────────

  it("inspects string variables", function()
    local filepath = tmpdir .. "/debug_stepping.py"
    vim.cmd("edit " .. vim.fn.fnameescape(filepath))
    local buf = vim.api.nvim_get_current_buf()
    lsp_helpers.wait_for_server_ready(buf)

    -- Breakpoint on line 21 (greeting = "hello").
    vim.api.nvim_win_set_cursor(0, { 21, 0 })
    dap.toggle_breakpoint()

    dap.run({
      type = "basilisk",
      request = "launch",
      name = "Test: strings",
      program = filepath,
      justMyCode = true,
    })

    dap_helpers.wait_for_stopped()

    -- Step past greeting, name, message.
    dap_helpers.step_and_wait("next") -- greeting = "hello"
    dap_helpers.step_and_wait("next") -- name = "world"
    dap_helpers.step_and_wait("next") -- message = ...

    local vars = dap_helpers.get_local_variables()
    assert.are.equal("'hello'", vars["greeting"])
    assert.are.equal("'world'", vars["name"])
    assert.are.equal("'hello world'", vars["message"])
  end)

  -- ── Exception handling ──────────────────────────────────────────────

  it("inspects variables after exception", function()
    local filepath = tmpdir .. "/debug_stepping.py"
    vim.cmd("edit " .. vim.fn.fnameescape(filepath))
    local buf = vim.api.nvim_get_current_buf()
    lsp_helpers.wait_for_server_ready(buf)

    -- Breakpoint on line 93 (return caught).
    vim.api.nvim_win_set_cursor(0, { 93, 0 })
    dap.toggle_breakpoint()

    dap.run({
      type = "basilisk",
      request = "launch",
      name = "Test: exception",
      program = filepath,
      justMyCode = true,
    })

    local stopped = dap_helpers.wait_for_stopped()
    assert.is_true(stopped, "did not stop at breakpoint")

    local vars = dap_helpers.get_local_variables()
    assert.are.equal("True", vars["caught"])
  end)

  -- ── Stack trace ─────────────────────────────────────────────────────

  it("shows correct stack trace in nested calls", function()
    local filepath = tmpdir .. "/debug_stepping.py"
    vim.cmd("edit " .. vim.fn.fnameescape(filepath))
    local buf = vim.api.nvim_get_current_buf()
    lsp_helpers.wait_for_server_ready(buf)

    -- Breakpoint inside double() on line 59.
    vim.api.nvim_win_set_cursor(0, { 59, 0 })
    dap.toggle_breakpoint()

    dap.run({
      type = "basilisk",
      request = "launch",
      name = "Test: stack trace",
      program = filepath,
      justMyCode = true,
    })

    local stopped = dap_helpers.wait_for_stopped()
    assert.is_true(stopped, "did not stop at breakpoint")

    local frames = dap_helpers.get_stack_trace()
    assert.is_true(#frames >= 2, "expected at least 2 stack frames")
    assert.are.equal("double", frames[1].name)
  end)

  -- ── Evaluate expressions ────────────────────────────────────────────

  it("evaluates expressions in debug console", function()
    local filepath = tmpdir .. "/debug_stepping.py"
    vim.cmd("edit " .. vim.fn.fnameescape(filepath))
    local buf = vim.api.nvim_get_current_buf()
    lsp_helpers.wait_for_server_ready(buf)

    -- Breakpoint on line 15 (result = w - 5) — x, y, z, w are all set.
    vim.api.nvim_win_set_cursor(0, { 15, 0 })
    dap.toggle_breakpoint()

    dap.run({
      type = "basilisk",
      request = "launch",
      name = "Test: evaluate",
      program = filepath,
      justMyCode = true,
    })

    dap_helpers.wait_for_stopped()

    -- Evaluate arithmetic expressions.
    local result = dap_helpers.evaluate("x + y")
    assert.are.equal("30", result)

    result = dap_helpers.evaluate("z * 2")
    assert.are.equal("60", result)

    -- Evaluate a type check.
    result = dap_helpers.evaluate("type(x).__name__")
    assert.are.equal("'int'", result)
  end)

  -- ── Clean termination ───────────────────────────────────────────────

  it("terminates cleanly after program completes", function()
    local filepath = tmpdir .. "/debug_stepping.py"
    vim.cmd("edit " .. vim.fn.fnameescape(filepath))
    local buf = vim.api.nvim_get_current_buf()
    lsp_helpers.wait_for_server_ready(buf)

    -- No breakpoints — let the program run to completion.
    dap.run({
      type = "basilisk",
      request = "launch",
      name = "Test: clean termination",
      program = filepath,
      justMyCode = true,
    })

    local session_active = dap_helpers.wait_for_session()
    assert.is_true(session_active, "debug session did not start")

    -- Wait for the session to terminate naturally.
    local terminated = dap_helpers.wait_for_terminated()
    assert.is_true(terminated, "debug session did not terminate after program completed")
  end)
end)
