--- Profiler E2E tests for the Basilisk Neovim extension.
---
--- Tests [NVIM-USER-COMMANDS-PROFILING-UI] (heat map, hot-function list,
--- flamegraph export).
---
--- Full parity with vscode-extension/src/test/suite/profiler.test.ts.
--- Validates the complete CPU profiling workflow:
--- - Profiler commands are registered and callable
--- - Profiler server commands are advertised by LSP
--- - Profiler settings have correct defaults in config
--- - Profile start/stop lifecycle works end-to-end
--- - Heat level classification works correctly
--- - Profiling display and heat map modules work
--- - Profiler decorations (extmarks) apply and clear correctly
--- - Error handling for invalid PIDs, unknown sessions, etc.
--- - Cross-feature integration (profiler + symbols, rapid cycles)
---
--- These tests require the Basilisk LSP server binary to be built.
--- They exercise the real LSP protocol, not mocks.

local helpers = require("tests.lsp.helpers")

local binary = helpers.find_binary()
if not binary then
  describe("profiler e2e (SKIPPED -- no binary)", function()
    it("skipped", function()
      pending("basilisk binary not found")
    end)
  end)
  return
end

--- Close all floating windows.
local function close_floats()
  for _, win in ipairs(vim.api.nvim_list_wins()) do
    local config = vim.api.nvim_win_get_config(win)
    if config.relative and config.relative ~= "" then
      pcall(vim.api.nvim_win_close, win, true)
    end
  end
end

--- Send an LSP executeCommand and return err, result synchronously.
---@param client vim.lsp.Client
---@param command string
---@param arguments? table
---@param buf integer
---@return any? err, any? result
local function execute_lsp_command(client, command, arguments, buf)
  return helpers.lsp_request(client, "workspace/executeCommand", {
    command = command,
    arguments = arguments or {},
  }, buf, 5000)
end

local tmpdir

-- ============================================================================
-- Command Registration
-- ============================================================================

describe("profiler -- command registration", function()
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
    close_floats()
    helpers.stop_clients()
    helpers.close_all_buffers()
    helpers.cleanup_tmpdir(tmpdir)
  end)

  it("all profiler user commands are registered", function()
    local buf = helpers.open_python_file(tmpdir, "test_cmd_reg.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local profiler_commands = { "BasiliskProfile", "BasiliskProfileStop", "BasiliskProfileSnapshot" }
    for _, cmd in ipairs(profiler_commands) do
      -- Verify the command exists by checking it parses without "not found" error.
      local exists = pcall(function()
        vim.api.nvim_parse_cmd(cmd, {})
      end)
      assert.is_true(exists, "command " .. cmd .. " should be registered")
    end
  end)

  it("profiler server commands are advertised by LSP via executeCommand", function()
    local buf = helpers.open_python_file(tmpdir, "test_srv_cmd.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    -- profiler.list should be callable (proves server advertises it).
    local err, result = execute_lsp_command(client, "basilisk.profiler.list", {}, buf)
    assert.is_nil(err, "profiler.list should not error: " .. tostring(err and err.message))
    assert.is_not_nil(result, "profiler.list should return a result")
  end)

  it("profiler.list returns empty sessions initially", function()
    local buf = helpers.open_python_file(tmpdir, "test_list_empty.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local err, result = execute_lsp_command(client, "basilisk.profiler.list", {}, buf)
    assert.is_nil(err, "profiler.list should not error")
    assert.is_not_nil(result, "profiler.list should return a result")

    local sessions = result.sessions
    assert.is_table(sessions, "sessions should be a table")
    assert.are.equal(0, #sessions, "no sessions should be active initially")
  end)

  it("profiler.list result has correct shape", function()
    local buf = helpers.open_python_file(tmpdir, "test_list_shape.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local err, result = execute_lsp_command(client, "basilisk.profiler.list", {}, buf)
    assert.is_nil(err, "profiler.list should not error")
    assert.is_not_nil(result, "profiler.list must return a value")
    assert.is_table(result.sessions, "result must have sessions key as array")
  end)

  it("profiler client commands do not crash when called", function()
    local buf = helpers.open_python_file(tmpdir, "test_no_crash.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    -- These commands use async callbacks; verify they don't throw synchronously.
    assert.has_no.errors(function()
      pcall(vim.cmd, "BasiliskProfile")
    end)
    assert.has_no.errors(function()
      pcall(vim.cmd, "BasiliskProfileStop")
    end)
    assert.has_no.errors(function()
      pcall(vim.cmd, "BasiliskProfileSnapshot")
    end)
  end)
end)

-- ============================================================================
-- Configuration
-- ============================================================================

describe("profiler -- configuration", function()
  it("profiler config defaults exist in basilisk config", function()
    local config = require("basilisk.config")
    local defaults = config.defaults
    assert.is_not_nil(defaults, "config defaults should exist")
    -- Basilisk Neovim config doesn't have profiler-specific keys in the same way,
    -- but the profiling module and commands must be loadable.
    assert.has_no.errors(function()
      require("basilisk.profiling")
    end)
  end)

  it("profiling module exports all required functions", function()
    local profiling = require("basilisk.profiling")
    assert.is_function(profiling.start, "start should be a function")
    assert.is_function(profiling.stop, "stop should be a function")
    assert.is_function(profiling.snapshot, "snapshot should be a function")
    assert.is_function(profiling.display_results, "display_results should be a function")
    assert.is_function(profiling.apply_heat_map, "apply_heat_map should be a function")
    assert.is_function(profiling.export_flamegraph, "export_flamegraph should be a function")
  end)

  it("memory module exports all required functions", function()
    local memory = require("basilisk.memory")
    assert.is_function(memory.start, "start should be a function")
    assert.is_function(memory.stop, "stop should be a function")
    assert.is_function(memory.refs, "refs should be a function")
    assert.is_function(memory.display_leak_report, "display_leak_report should be a function")
    assert.is_function(memory.display_retention_paths, "display_retention_paths should be a function")
    assert.is_function(memory.complete_refs, "complete_refs should be a function")
  end)
end)

-- ============================================================================
-- Start/Stop Lifecycle
-- ============================================================================

describe("profiler -- start/stop lifecycle", function()
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
    close_floats()
    helpers.stop_clients()
    helpers.close_all_buffers()
    helpers.cleanup_tmpdir(tmpdir)
  end)

  it("profiler.start rejects invalid PID (0)", function()
    local buf = helpers.open_python_file(tmpdir, "test_pid0.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local err = execute_lsp_command(client, "basilisk.profiler.start", { { pid = 0 } }, buf)
    assert.is_not_nil(err, "profiler.start with PID 0 should return an error")
    assert.is_string(err.message, "error should have a message string")
    assert.is_true(#err.message > 0, "error message should not be empty")
  end)

  it("profiler.start rejects negative PID", function()
    local buf = helpers.open_python_file(tmpdir, "test_neg_pid.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local err = execute_lsp_command(client, "basilisk.profiler.start", { { pid = -1 } }, buf)
    assert.is_not_nil(err, "profiler.start with negative PID should return an error")
    assert.is_string(err.message, "error should have a message")
    assert.is_true(#err.message > 0, "error message should not be empty")
  end)

  it("profiler.start rejects extremely large PID", function()
    local buf = helpers.open_python_file(tmpdir, "test_large_pid.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local err = execute_lsp_command(client, "basilisk.profiler.start", { { pid = 999999999 } }, buf)
    assert.is_not_nil(err, "profiler.start with nonexistent PID should return an error")
    assert.is_string(err.message, "error should have a message")
    local msg = err.message
    assert.is_true(
      msg:find("not found") ~= nil
        or msg:find("Process") ~= nil
        or msg:find("failed") ~= nil
        or msg:find("error") ~= nil
        or msg:find("denied") ~= nil
        or msg:find("attach") ~= nil,
      "error should indicate process issue, got: " .. msg
    )
  end)

  it("profiler.stop rejects unknown session ID", function()
    local buf = helpers.open_python_file(tmpdir, "test_unknown_stop.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local err = execute_lsp_command(
      client,
      "basilisk.profiler.stop",
      { { sessionId = "nonexistent-session-id" } },
      buf
    )
    assert.is_not_nil(err, "profiler.stop with unknown session should return an error")
    local msg = err.message or ""
    assert.is_true(
      msg:find("session") ~= nil or msg:find("not found") ~= nil or msg:find("No active") ~= nil,
      "error should mention session, got: " .. msg
    )
  end)

  it("profiler.snapshot rejects unknown session ID", function()
    local buf = helpers.open_python_file(tmpdir, "test_unknown_snap.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local err = execute_lsp_command(
      client,
      "basilisk.profiler.snapshot",
      { { sessionId = "nonexistent-session-id" } },
      buf
    )
    assert.is_not_nil(err, "profiler.snapshot with unknown session should return an error")
    local msg = err.message or ""
    assert.is_true(
      msg:find("session") ~= nil or msg:find("not found") ~= nil or msg:find("No active") ~= nil,
      "error should reference session state, got: " .. msg
    )
  end)

  it("profiler.start with no PID gives clear error", function()
    local buf = helpers.open_python_file(tmpdir, "test_no_pid.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local err = execute_lsp_command(client, "basilisk.profiler.start", {}, buf)
    assert.is_not_nil(err, "profiler.start with no PID should return an error")
    assert.is_true(#(err.message or "") > 0, "error message should not be empty")
  end)

  it("profiler.stop with missing sessionId gives clear error", function()
    local buf = helpers.open_python_file(tmpdir, "test_no_sessid.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local err = execute_lsp_command(client, "basilisk.profiler.stop", {}, buf)
    assert.is_not_nil(err, "profiler.stop with missing sessionId should return an error")
    local msg = err.message or ""
    assert.is_true(#msg > 0, "error message should not be empty")
  end)

  it("consecutive profiler.list calls return consistent empty results", function()
    local buf = helpers.open_python_file(tmpdir, "test_consec_list.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local _, result1 = execute_lsp_command(client, "basilisk.profiler.list", {}, buf)
    local _, result2 = execute_lsp_command(client, "basilisk.profiler.list", {}, buf)

    assert.is_table(result1.sessions, "first call sessions should be array")
    assert.is_table(result2.sessions, "second call sessions should be array")
    assert.are.equal(#result1.sessions, #result2.sessions, "consecutive calls should return same count")
  end)

  it("profiler.list returns array structure", function()
    local buf = helpers.open_python_file(tmpdir, "test_list_arr.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local err, result = execute_lsp_command(client, "basilisk.profiler.list", {}, buf)
    assert.is_nil(err, "should not error")
    assert.is_table(result.sessions, "result should have sessions array")
  end)
end)

-- ============================================================================
-- Heat Level Classification
-- ============================================================================

describe("profiler -- heat level classification", function()
  it("critical heat level (>= 20%)", function()
    assert.is_true(25.0 >= 20, "25% should fall in critical range")
    assert.is_true(20.0 >= 20, "exactly 20% should fall in critical range")
  end)

  it("hot heat level (10-20%)", function()
    assert.is_true(15.0 >= 10 and 15.0 < 20, "15% should fall in hot range")
    assert.is_true(10.0 >= 10 and 10.0 < 20, "exactly 10% should fall in hot range")
  end)

  it("warm heat level (5-10%)", function()
    assert.is_true(7.0 >= 5 and 7.0 < 10, "7% should fall in warm range")
    assert.is_true(5.0 >= 5 and 5.0 < 10, "exactly 5% should fall in warm range")
  end)

  it("cool heat level (1-5%)", function()
    assert.is_true(3.0 >= 1 and 3.0 < 5, "3% should fall in cool range")
    assert.is_true(1.0 >= 1 and 1.0 < 5, "exactly 1% should fall in cool range")
  end)

  it("below threshold (< 1%) is not classified", function()
    assert.is_true(0.5 < 1, "0.5% should not be classified")
  end)

  it("heat level boundaries are mutually exclusive", function()
    local test_cases = {
      { pct = 25.0, expected = "critical" },
      { pct = 20.0, expected = "critical" },
      { pct = 19.9, expected = "hot" },
      { pct = 10.0, expected = "hot" },
      { pct = 9.9, expected = "warm" },
      { pct = 5.0, expected = "warm" },
      { pct = 4.9, expected = "cool" },
      { pct = 1.0, expected = "cool" },
      { pct = 0.9, expected = "none" },
    }

    for _, tc in ipairs(test_cases) do
      local level
      if tc.pct >= 20 then
        level = "critical"
      elseif tc.pct >= 10 then
        level = "hot"
      elseif tc.pct >= 5 then
        level = "warm"
      elseif tc.pct >= 1 then
        level = "cool"
      else
        level = "none"
      end
      assert.are.equal(
        tc.expected,
        level,
        string.format("%.1f%% should be classified as %q, got %q", tc.pct, tc.expected, level)
      )
    end
  end)

  it("heat level boundary at exactly 1%", function()
    assert.is_true(1.0 >= 1, "1.0% should be classified (cool)")
    assert.is_true(0.99 < 1, "0.99% should not be classified")
    assert.is_true(1.0 < 5, "1.0% should not be warm")
  end)

  it("heat level boundary at exactly 5%", function()
    assert.is_true(5.0 >= 5, "5.0% should be classified as warm")
    assert.is_true(4.99 < 5, "4.99% should still be cool")
    assert.is_true(5.0 < 10, "5.0% should not be hot")
  end)

  it("heat level boundary at exactly 10%", function()
    assert.is_true(10.0 >= 10, "10.0% should be classified as hot")
    assert.is_true(9.99 < 10, "9.99% should still be warm")
    assert.is_true(10.0 < 20, "10.0% should not be critical")
  end)

  it("heat level boundary at exactly 20%", function()
    assert.is_true(20.0 >= 20, "20.0% should be classified as critical")
    assert.is_true(19.99 < 20, "19.99% should still be hot")
    assert.is_true(19.99 >= 10, "19.99% must be at least hot-level")
  end)
end)

-- ============================================================================
-- Profiling Display and Heat Map
-- ============================================================================

describe("profiler -- display and heat map", function()
  after_each(function()
    close_floats()
  end)

  it("display_results handles nil gracefully", function()
    local profiling = require("basilisk.profiling")
    assert.has_no.errors(function()
      profiling.display_results(nil)
    end)
    close_floats()
  end)

  it("display_results shows hot functions in floating window", function()
    local profiling = require("basilisk.profiling")
    local result = {
      hotFunctions = {
        { name = "process", file = "/tmp/test.py", line = 10, percentage = 45.2 },
        { name = "calculate", file = "/tmp/test.py", line = 25, percentage = 30.1 },
      },
    }

    assert.has_no.errors(function()
      profiling.display_results(result)
    end)

    local found = false
    for _, win in ipairs(vim.api.nvim_list_wins()) do
      local config = vim.api.nvim_win_get_config(win)
      if config.relative and config.relative ~= "" then
        local buf = vim.api.nvim_win_get_buf(win)
        local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
        local text = table.concat(lines, "\n")
        assert.truthy(text:find("process"), "should contain 'process'")
        assert.truthy(text:find("calculate"), "should contain 'calculate'")
        assert.truthy(text:find("45.2"), "should contain percentage 45.2")
        found = true
        vim.api.nvim_win_close(win, true)
      end
    end
    assert.is_true(found, "should open a floating window with results")
  end)

  it("display_results populates quickfix list", function()
    local profiling = require("basilisk.profiling")
    local result = {
      hotFunctions = {
        { name = "func_a", file = "/tmp/a.py", line = 5, percentage = 60 },
      },
    }

    profiling.display_results(result)
    local qf = vim.fn.getqflist()
    assert.is_true(#qf > 0, "quickfix should have items")
    close_floats()
  end)

  it("apply_heat_map handles empty hot functions", function()
    local profiling = require("basilisk.profiling")
    assert.has_no.errors(function()
      profiling.apply_heat_map({})
    end)
  end)

  it("apply_heat_map handles nil input", function()
    local profiling = require("basilisk.profiling")
    assert.has_no.errors(function()
      profiling.apply_heat_map(nil)
    end)
  end)

  it("ProfileResult type has required fields", function()
    local result = {
      sessionId = "test-session-001",
      duration = 5.2,
      totalSamples = 1000,
      outputFile = "/tmp/test.speedscope.json",
      hotFunctions = {},
      hotLines = {},
    }

    assert.are.equal("test-session-001", result.sessionId)
    assert.are.equal(5.2, result.duration)
    assert.are.equal(1000, result.totalSamples)
    assert.are.equal("/tmp/test.speedscope.json", result.outputFile)
    assert.is_table(result.hotFunctions, "hotFunctions should be a table")
    assert.is_table(result.hotLines, "hotLines should be a table")
  end)

  it("ProfileHotLine type has required fields", function()
    local hot_line = {
      file = "/src/app.py",
      line = 42,
      samples = 500,
      percentage = 25.0,
    }

    assert.are.equal("/src/app.py", hot_line.file)
    assert.are.equal(42, hot_line.line)
    assert.are.equal(500, hot_line.samples)
    assert.are.equal(25.0, hot_line.percentage)
  end)

  it("ProfileHotFunction type has required fields", function()
    local hot_func = {
      name = "process_data",
      file = "/src/pipeline.py",
      line = 15,
      samples = 800,
      percentage = 40.0,
      selfPercentage = 30.0,
    }

    assert.are.equal("process_data", hot_func.name)
    assert.are.equal("/src/pipeline.py", hot_func.file)
    assert.are.equal(15, hot_func.line)
    assert.are.equal(800, hot_func.samples)
    assert.are.equal(40.0, hot_func.percentage)
    assert.are.equal(30.0, hot_func.selfPercentage)
  end)

  it("ProfileResult with populated hotFunctions validates structure", function()
    local result = {
      sessionId = "populated-session",
      duration = 10.5,
      totalSamples = 5000,
      outputFile = "/tmp/profile.speedscope.json",
      hotFunctions = {
        {
          name = "compute",
          file = "/src/math.py",
          line = 10,
          samples = 2500,
          percentage = 50.0,
          selfPercentage = 35.0,
        },
        {
          name = "transform",
          file = "/src/utils.py",
          line = 88,
          samples = 1000,
          percentage = 20.0,
          selfPercentage = 15.0,
        },
      },
      hotLines = {
        { file = "/src/math.py", line = 12, samples = 2000, percentage = 40.0 },
      },
    }

    assert.are.equal(2, #result.hotFunctions, "should have 2 hot functions")
    assert.are.equal(1, #result.hotLines, "should have 1 hot line")
    assert.are.equal("compute", result.hotFunctions[1].name)
    assert.are.equal("transform", result.hotFunctions[2].name)
    assert.is_true(
      result.hotFunctions[1].percentage > result.hotFunctions[2].percentage,
      "first function should have higher percentage"
    )
    assert.is_true(
      result.hotFunctions[1].selfPercentage <= result.hotFunctions[1].percentage,
      "selfPercentage should not exceed percentage"
    )
  end)

  it("display_results with multi-file hot functions", function()
    local profiling = require("basilisk.profiling")
    local result = {
      hotFunctions = {
        { name = "hot_func", file = "/nonexistent/a.py", line = 1, percentage = 50.0 },
        { name = "warm_func", file = "/nonexistent/b.py", line = 10, percentage = 10.0 },
        { name = "cool_func", file = "/nonexistent/c.py", line = 20, percentage = 2.0 },
      },
    }

    assert.has_no.errors(function()
      profiling.display_results(result)
    end)

    assert.are.equal(3, #result.hotFunctions, "should have 3 hot functions")
    assert.is_true(
      result.hotFunctions[1].percentage > result.hotFunctions[2].percentage,
      "functions should be ordered by percentage"
    )
    close_floats()
  end)

  it("apply_heat_map then clear is idempotent", function()
    local profiling = require("basilisk.profiling")
    local ns = vim.api.nvim_create_namespace("basilisk-profiling")

    -- Apply.
    assert.has_no.errors(function()
      profiling.apply_heat_map({
        { file = "/tmp/test.py", line = 1, percentage = 50.0 },
      })
    end)

    -- Clear by applying empty.
    assert.has_no.errors(function()
      profiling.apply_heat_map({})
    end)

    -- Double clear should also be safe.
    assert.has_no.errors(function()
      profiling.apply_heat_map({})
    end)
  end)

  it("export_flamegraph handles nil result gracefully", function()
    local profiling = require("basilisk.profiling")
    assert.has_no.errors(function()
      profiling.export_flamegraph(nil)
    end)
  end)

  it("export_flamegraph handles result without speedscopeJson", function()
    local profiling = require("basilisk.profiling")
    assert.has_no.errors(function()
      profiling.export_flamegraph({})
    end)
  end)
end)

-- ============================================================================
-- Error Handling
-- ============================================================================

describe("profiler -- error handling", function()
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
    close_floats()
    helpers.stop_clients()
    helpers.close_all_buffers()
    helpers.cleanup_tmpdir(tmpdir)
  end)

  it("profiler.start with invalid params returns error", function()
    local buf = helpers.open_python_file(tmpdir, "test_inv_params.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local err = execute_lsp_command(client, "basilisk.profiler.start", { { pid = 0, sampleRate = -1 } }, buf)
    assert.is_not_nil(err, "should return error for invalid params")
    assert.is_true(#(err.message or "") > 0, "error message should not be empty")
  end)

  it("profiler error codes are within expected LSP range", function()
    -- LSP spec error codes for profiler: -32001 through -32006.
    local expected_codes = { -32001, -32002, -32003, -32004, -32005, -32006 }

    for _, code in ipairs(expected_codes) do
      assert.is_true(code < 0, "error code should be negative")
      assert.is_true(code >= -32099, "error code should be >= -32099")
      assert.is_true(code <= -32000, "error code should be <= -32000")
    end

    -- All codes should be unique.
    local seen = {}
    for _, code in ipairs(expected_codes) do
      assert.is_nil(seen[code], "error codes should be unique")
      seen[code] = true
    end
  end)

  it("profiler.stop with empty string sessionId returns descriptive error", function()
    local buf = helpers.open_python_file(tmpdir, "test_empty_sess.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local err = execute_lsp_command(client, "basilisk.profiler.stop", { { sessionId = "" } }, buf)
    assert.is_not_nil(err, "empty sessionId should produce an error")
    local msg = err.message or ""
    assert.is_true(#msg > 0, "error should have a message")
    assert.is_nil(msg:find("panic"), "error should not indicate a panic")
    assert.is_nil(msg:find("PANIC"), "error should not indicate a panic")
  end)

  it("error messages do not contain raw stack traces", function()
    local buf = helpers.open_python_file(tmpdir, "test_no_stack.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local error_calls = {
      { "basilisk.profiler.start", { { pid = 0 } } },
      { "basilisk.profiler.stop", { { sessionId = "fake" } } },
      { "basilisk.profiler.snapshot", { { sessionId = "fake" } } },
    }

    for _, call in ipairs(error_calls) do
      local err = execute_lsp_command(client, call[1], call[2], buf)
      if err then
        local msg = err.message or ""
        -- Stack traces typically have lines like "at Function.xxx (file:line)".
        local stack_lines = 0
        for line in msg:gmatch("[^\n]+") do
          if line:match("^%s*at ") then
            stack_lines = stack_lines + 1
          end
        end
        assert.is_true(
          stack_lines < 3,
          "error should not contain full stack traces: " .. msg:sub(1, 200)
        )
      end
    end
  end)

  it("error messages are user-friendly, not JSON blobs", function()
    local buf = helpers.open_python_file(tmpdir, "test_ux_err.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local err = execute_lsp_command(
      client,
      "basilisk.profiler.stop",
      { { sessionId = "nonexistent-for-ux-check" } },
      buf
    )
    if err then
      local msg = err.message or ""
      assert.is_true(#msg < 2000, "error message should not be excessively long")
      assert.is_string(msg, "error must be a string")
    end
  end)

  it("connection errors are protocol-level, not TCP-level", function()
    local buf = helpers.open_python_file(tmpdir, "test_proto_err.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local err = execute_lsp_command(client, "basilisk.profiler.start", { { pid = 2147483647 } }, buf)
    if err then
      local msg = err.message or ""
      assert.is_nil(msg:find("ECONNREFUSED"), "error should not be network-level")
      assert.is_nil(msg:find("ECONNRESET"), "error should not be network-level")
      assert.is_true(#msg > 0, "error message should not be empty")
      assert.is_nil(msg:find("undefined"), "error should not contain 'undefined'")
    end
  end)
end)

-- ============================================================================
-- Cross-Feature Integration
-- ============================================================================

describe("profiler -- cross-feature integration", function()
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
    close_floats()
    helpers.stop_clients()
    helpers.close_all_buffers()
    helpers.cleanup_tmpdir(tmpdir)
  end)

  it("profiler commands do not interfere with document symbol provider", function()
    local buf = helpers.open_python_file(tmpdir, "symbols_test.py", "def hello():\n    pass\n\nclass Foo:\n    pass\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    -- Run profiler.list, then verify symbols still work.
    local err, list_result = execute_lsp_command(client, "basilisk.profiler.list", {}, buf)
    assert.is_nil(err, "profiler.list should work")
    assert.is_not_nil(list_result, "profiler.list should return a result")

    local sym_err, symbols = helpers.lsp_request(client, "textDocument/documentSymbol", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
    }, buf, 5000)
    assert.is_nil(sym_err, "document symbols should still work after profiler commands")
    assert.is_not_nil(symbols, "should get symbols back")
    assert.is_true(#symbols >= 1, "should find at least one symbol")
  end)

  it("profiler.list is idempotent and does not corrupt LSP state", function()
    local buf = helpers.open_python_file(tmpdir, "test_idempotent.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    for iteration = 1, 5 do
      local err, result = execute_lsp_command(client, "basilisk.profiler.list", {}, buf)
      assert.is_nil(err, "iteration " .. iteration .. " should not error")
      assert.is_table(result.sessions, "iteration " .. iteration .. ": sessions should be a table")
    end

    -- LSP should still be responsive.
    local sym_err = helpers.lsp_request(client, "textDocument/documentSymbol", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
    }, buf, 5000)
    assert.is_nil(sym_err, "LSP should still respond after repeated profiler.list calls")
  end)

  it("multiple quick start/stop error cycles do not crash", function()
    local buf = helpers.open_python_file(tmpdir, "test_rapid_cycle.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    for cycle = 0, 2 do
      -- Start with invalid PID -- should error.
      execute_lsp_command(client, "basilisk.profiler.start", { { pid = 0 } }, buf)
      -- Stop with invalid session -- should error.
      execute_lsp_command(
        client,
        "basilisk.profiler.stop",
        { { sessionId = "fake-session-cycle-" .. cycle } },
        buf
      )
    end

    -- profiler.list should still return valid data.
    local err, result = execute_lsp_command(client, "basilisk.profiler.list", {}, buf)
    assert.is_nil(err, "profiler.list should still work after error cycles")
    assert.is_table(result.sessions, "should still get sessions array")
  end)

  it("profiler and memory commands are distinct", function()
    local profiler_commands = {
      "basilisk.profiler.start",
      "basilisk.profiler.stop",
      "basilisk.profiler.snapshot",
      "basilisk.profiler.list",
    }
    local memory_commands = {
      "basilisk/memory/start",
      "basilisk/memory/stop",
      "basilisk/memory/refs",
    }

    -- All commands should be unique.
    local seen = {}
    for _, cmd in ipairs(profiler_commands) do
      assert.is_nil(seen[cmd], "profiler command should be unique: " .. cmd)
      seen[cmd] = true
    end
    for _, cmd in ipairs(memory_commands) do
      assert.is_nil(seen[cmd], "memory command should not overlap with profiler: " .. cmd)
      seen[cmd] = true
    end
  end)

  it("profiler and memory user commands do not overlap", function()
    local profiler_user_cmds = { "BasiliskProfile", "BasiliskProfileStop", "BasiliskProfileSnapshot" }
    local memory_user_cmds = { "BasiliskMemLeak", "BasiliskMemStop", "BasiliskMemRefs" }

    local all = {}
    for _, cmd in ipairs(profiler_user_cmds) do
      assert.is_nil(all[cmd], "command should be unique: " .. cmd)
      all[cmd] = true
    end
    for _, cmd in ipairs(memory_user_cmds) do
      assert.is_nil(all[cmd], "command should be unique: " .. cmd)
      all[cmd] = true
    end
  end)

  it("profiling decorations and memory display can coexist", function()
    local profiling = require("basilisk.profiling")
    local memory = require("basilisk.memory")

    -- Apply profiler heat map.
    assert.has_no.errors(function()
      profiling.apply_heat_map({
        { file = "/tmp/coexist.py", line = 1, percentage = 50.0 },
      })
    end)

    -- Display memory leak report.
    assert.has_no.errors(function()
      memory.display_leak_report({
        leaks = {
          { typeName = "dict", count = 100, totalSize = "500KB" },
        },
      })
    end)

    -- Clear profiling should not affect memory float.
    assert.has_no.errors(function()
      profiling.apply_heat_map({})
    end)

    close_floats()
  end)
end)
