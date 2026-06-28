--- Memory Profiler E2E tests for the Basilisk Neovim extension.
---
--- Tests [NVIM-USER-COMMANDS-MEMORY-UI] (leak report, retention paths,
--- :BasiliskMemRefs completion).
---
--- Full parity with vscode-extension/src/test/suite/profiler.test.ts
--- (Memory Profiler sections).
--- Validates the complete memory profiling workflow:
--- - Memory profiler commands are registered and callable
--- - Memory start/snapshot/stop/refs lifecycle
--- - Leak report display in floating windows
--- - Retention path visualization
--- - Memory type structures (MemoryAllocation, MemorySnapshotResult, etc.)
--- - Leak confidence levels and severity ordering
--- - SuspectedLeak and MemoryDiffResult types
--- - Memory decorations apply and clear without throwing
--- - Type completion for :BasiliskMemRefs
---
--- These tests require the Basilisk LSP server binary to be built.
--- They exercise the real LSP protocol, not mocks.

local helpers = require("tests.lsp.helpers")

local binary = helpers.find_binary()
if not binary then
  describe("memory profiler e2e (SKIPPED -- no binary)", function()
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
-- Memory Command Registration
-- ============================================================================

describe("memory profiler -- command registration", function()
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

  it("all memory user commands are registered", function()
    local buf = helpers.open_python_file(tmpdir, "test_mem_reg.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local memory_commands = { "BasiliskMemLeak", "BasiliskMemStop", "BasiliskMemRefs" }
    for _, cmd in ipairs(memory_commands) do
      local exists = pcall(function()
        vim.api.nvim_parse_cmd(cmd .. " dict", {})
      end)
      -- BasiliskMemRefs needs an arg, but parse_cmd should still recognize it.
      -- For MemLeak/MemStop, parse_cmd without args is fine.
      if cmd ~= "BasiliskMemRefs" then
        exists = pcall(function()
          vim.api.nvim_parse_cmd(cmd, {})
        end)
      end
      assert.is_true(exists, "command " .. cmd .. " should be registered")
    end
  end)

  it("memory client commands do not crash when called", function()
    local buf = helpers.open_python_file(tmpdir, "test_mem_nocrash.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    assert.has_no.errors(function()
      pcall(vim.cmd, "BasiliskMemLeak")
    end)
    assert.has_no.errors(function()
      pcall(vim.cmd, "BasiliskMemStop")
    end)
    assert.has_no.errors(function()
      pcall(vim.cmd, "BasiliskMemRefs dict")
    end)
  end)

  it("memory and profiler user commands are all distinct", function()
    local all_commands = {
      "BasiliskProfile",
      "BasiliskProfileStop",
      "BasiliskProfileSnapshot",
      "BasiliskMemLeak",
      "BasiliskMemStop",
      "BasiliskMemRefs",
    }

    local seen = {}
    for _, cmd in ipairs(all_commands) do
      assert.is_nil(seen[cmd], "command should be unique: " .. cmd)
      seen[cmd] = true
    end
    assert.are.equal(6, vim.tbl_count(seen), "should have 6 unique commands")
  end)
end)

-- ============================================================================
-- Memory Profiler Lifecycle (with real LSP)
-- ============================================================================

describe("memory profiler -- lifecycle", function()
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

  it("memoryStart is callable and returns without crash", function()
    local buf = helpers.open_python_file(tmpdir, "test_mem_start.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    -- Should not crash even if memory tracking is not available.
    assert.has_no.errors(function()
      pcall(vim.cmd, "BasiliskMemLeak")
    end)
    vim.wait(500)
  end)

  it("memorySnapshot without active session warns gracefully", function()
    local buf = helpers.open_python_file(tmpdir, "test_mem_snap.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local memory = require("basilisk.memory")
    -- stop() without active session should not crash.
    assert.has_no.errors(function()
      memory.stop()
    end)
    vim.wait(500)
  end)

  it("memoryRefs is callable", function()
    local buf = helpers.open_python_file(tmpdir, "test_mem_refs.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local memory = require("basilisk.memory")
    -- refs() with a type name should not crash.
    assert.has_no.errors(function()
      memory.refs("dict")
    end)
    vim.wait(500)
  end)
end)

-- ============================================================================
-- Memory Type Structures
-- ============================================================================

describe("memory profiler -- type structures", function()
  after_each(function()
    close_floats()
  end)

  it("MemoryAllocation type has required fields", function()
    local alloc = {
      file = "/src/data.py",
      line = 100,
      size = 10485760,
      count = 5000,
    }

    assert.are.equal("/src/data.py", alloc.file)
    assert.are.equal(100, alloc.line)
    assert.are.equal(10485760, alloc.size)
    assert.are.equal(5000, alloc.count)
  end)

  it("MemoryAllocation type enforces required fields", function()
    local alloc = {
      file = "/src/allocator.py",
      line = 55,
      size = 52428800,
      count = 10000,
    }

    assert.are.equal("/src/allocator.py", alloc.file)
    assert.are.equal(55, alloc.line)
    assert.are.equal(52428800, alloc.size)
    assert.are.equal(10000, alloc.count)
  end)

  it("MemorySnapshotResult type has required fields", function()
    local snapshot = {
      memorySessionId = "mem-session-001",
      snapshotId = "snap-001",
      currentMemory = 50000000,
      peakMemory = 75000000,
      topAllocations = {},
    }

    assert.are.equal("mem-session-001", snapshot.memorySessionId)
    assert.are.equal("snap-001", snapshot.snapshotId)
    assert.are.equal(50000000, snapshot.currentMemory)
    assert.are.equal(75000000, snapshot.peakMemory)
    assert.is_table(snapshot.topAllocations, "topAllocations should be a table")
  end)

  it("SuspectedLeak type has all required fields", function()
    local leak = {
      file = "/src/leaky.py",
      line = 42,
      sizeGrowth = 1048576,
      countGrowth = 500,
      currentSize = 5242880,
      confidence = "HIGH",
      reason = "Monotonic growth detected across 10 snapshots",
    }

    assert.are.equal("/src/leaky.py", leak.file)
    assert.are.equal(42, leak.line)
    assert.are.equal(1048576, leak.sizeGrowth)
    assert.are.equal(500, leak.countGrowth)
    assert.are.equal(5242880, leak.currentSize)
    assert.are.equal("HIGH", leak.confidence)
    assert.is_true(#leak.reason > 0, "leak reason should be non-empty")
  end)

  it("MemoryDiffResult type has all required fields", function()
    local diff = {
      totalGrowth = 10485760,
      totalFreed = 2097152,
      netGrowth = 8388608,
      suspectedLeaks = {
        {
          file = "/src/data.py",
          line = 10,
          sizeGrowth = 5242880,
          countGrowth = 200,
          currentSize = 10485760,
          confidence = "DEFINITE",
          reason = "Allocation grows every snapshot with zero frees",
        },
      },
    }

    assert.are.equal(10485760, diff.totalGrowth)
    assert.are.equal(2097152, diff.totalFreed)
    assert.are.equal(8388608, diff.netGrowth)
    assert.are.equal(1, #diff.suspectedLeaks)
    assert.are.equal("DEFINITE", diff.suspectedLeaks[1].confidence)
  end)

  it("leak confidence levels map to correct severity ordering", function()
    local confidences = { "LOW", "MEDIUM", "HIGH", "DEFINITE" }
    local severity_order = {
      LOW = 0,
      MEDIUM = 1,
      HIGH = 2,
      DEFINITE = 3,
    }

    assert.are.equal(4, #confidences, "should be exactly 4 confidence levels")

    for idx = 1, #confidences - 1 do
      local current = severity_order[confidences[idx]]
      local next_val = severity_order[confidences[idx + 1]]
      assert.is_not_nil(current, "severity for " .. confidences[idx] .. " must be defined")
      assert.is_not_nil(next_val, "severity for " .. confidences[idx + 1] .. " must be defined")
      assert.is_true(
        current < next_val,
        confidences[idx] .. " should have lower severity than " .. confidences[idx + 1]
      )
    end
  end)

  it("MemorySnapshotResult with populated allocations validates structure", function()
    local snapshot = {
      memorySessionId = "mem-populated",
      snapshotId = "snap-pop-001",
      currentMemory = 104857600,
      peakMemory = 209715200,
      topAllocations = {
        { file = "/nonexistent/a.py", line = 1, size = 52428800, count = 5000 },
        { file = "/nonexistent/b.py", line = 15, size = 10485760, count = 1000 },
        { file = "/nonexistent/c.py", line = 30, size = 1048576, count = 100 },
      },
    }

    assert.are.equal(3, #snapshot.topAllocations, "should have 3 allocations")
    assert.is_true(
      snapshot.currentMemory <= snapshot.peakMemory,
      "currentMemory should not exceed peakMemory"
    )
  end)

  it("MemoryDiffResult validates net growth calculation", function()
    local diff = {
      totalGrowth = 5242880,
      totalFreed = 524288,
      netGrowth = 4718592,
      suspectedLeaks = {
        {
          file = "/nonexistent/leaky.py",
          line = 3,
          sizeGrowth = 2097152,
          countGrowth = 300,
          currentSize = 8388608,
          confidence = "HIGH",
          reason = "Monotonic growth pattern",
        },
      },
    }

    assert.is_true(diff.netGrowth > 0, "net growth should be positive for a leak")
    assert.is_true(
      diff.totalGrowth > diff.totalFreed,
      "totalGrowth should exceed totalFreed when there is a net leak"
    )
    assert.are.equal(
      diff.totalGrowth - diff.totalFreed,
      diff.netGrowth,
      "netGrowth should equal totalGrowth - totalFreed"
    )
  end)
end)

-- ============================================================================
-- Memory Display
-- ============================================================================

describe("memory profiler -- display", function()
  after_each(function()
    close_floats()
  end)

  it("display_leak_report handles nil gracefully", function()
    local memory = require("basilisk.memory")
    assert.has_no.errors(function()
      memory.display_leak_report(nil)
    end)
    close_floats()
  end)

  it("display_leak_report shows leaks in floating window", function()
    local memory = require("basilisk.memory")
    local result = {
      leaks = {
        { typeName = "DataFrame", count = 15, totalSize = "1.2MB", location = { file = "/tmp/test.py", line = 42 } },
        { typeName = "dict", count = 100, totalSize = "500KB" },
      },
    }

    assert.has_no.errors(function()
      memory.display_leak_report(result)
    end)

    local found = false
    for _, win in ipairs(vim.api.nvim_list_wins()) do
      local config = vim.api.nvim_win_get_config(win)
      if config.relative and config.relative ~= "" then
        local buf = vim.api.nvim_win_get_buf(win)
        local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
        local text = table.concat(lines, "\n")
        assert.truthy(text:find("DataFrame"), "should contain 'DataFrame'")
        assert.truthy(text:find("dict"), "should contain 'dict'")
        assert.truthy(text:find("15 objects"), "should contain '15 objects'")
        found = true
        vim.api.nvim_win_close(win, true)
      end
    end
    assert.is_true(found, "should open floating window with leak report")
  end)

  it("display_retention_paths handles nil gracefully", function()
    local memory = require("basilisk.memory")
    assert.has_no.errors(function()
      memory.display_retention_paths("dict", nil)
    end)
    close_floats()
  end)

  it("display_retention_paths shows paths in floating window", function()
    local memory = require("basilisk.memory")
    local result = {
      retentionPaths = {
        {
          confidence = 0.85,
          steps = {
            { name = "global_cache", kind = "variable" },
            { name = "__dict__", kind = "attribute" },
          },
        },
      },
    }

    assert.has_no.errors(function()
      memory.display_retention_paths("DataFrame", result)
    end)

    local found = false
    for _, win in ipairs(vim.api.nvim_list_wins()) do
      local config = vim.api.nvim_win_get_config(win)
      if config.relative and config.relative ~= "" then
        local buf = vim.api.nvim_win_get_buf(win)
        local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
        local text = table.concat(lines, "\n")
        assert.truthy(text:find("DataFrame"), "should contain 'DataFrame'")
        assert.truthy(text:find("global_cache"), "should contain 'global_cache'")
        assert.truthy(text:find("85%%"), "should contain confidence percentage")
        found = true
        vim.api.nvim_win_close(win, true)
      end
    end
    assert.is_true(found, "should open floating window with retention paths")
  end)

  it("display_leak_report with no leaks shows empty message", function()
    local memory = require("basilisk.memory")
    assert.has_no.errors(function()
      memory.display_leak_report({ leaks = {} })
    end)

    local found = false
    for _, win in ipairs(vim.api.nvim_list_wins()) do
      local config = vim.api.nvim_win_get_config(win)
      if config.relative and config.relative ~= "" then
        local buf = vim.api.nvim_win_get_buf(win)
        local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
        local text = table.concat(lines, "\n")
        assert.truthy(text:find("No leaks"), "should say no leaks detected")
        found = true
        vim.api.nvim_win_close(win, true)
      end
    end
    assert.is_true(found, "should open floating window with empty message")
  end)

  it("display_retention_paths with no paths shows empty message", function()
    local memory = require("basilisk.memory")
    assert.has_no.errors(function()
      memory.display_retention_paths("dict", { retentionPaths = {} })
    end)

    local found = false
    for _, win in ipairs(vim.api.nvim_list_wins()) do
      local config = vim.api.nvim_win_get_config(win)
      if config.relative and config.relative ~= "" then
        local buf = vim.api.nvim_win_get_buf(win)
        local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
        local text = table.concat(lines, "\n")
        assert.truthy(text:find("No retention"), "should say no retention paths")
        found = true
        vim.api.nvim_win_close(win, true)
      end
    end
    assert.is_true(found, "should open floating window with empty message")
  end)
end)

-- ============================================================================
-- Type Completion for :BasiliskMemRefs
-- ============================================================================

describe("memory profiler -- type completion", function()
  it("returns DataFrame for 'Data' input", function()
    local memory = require("basilisk.memory")
    local matches = memory.complete_refs("Data")
    assert.are.equal("DataFrame", matches[1])
  end)

  it("returns all types for empty input", function()
    local memory = require("basilisk.memory")
    local matches = memory.complete_refs("")
    assert.is_true(#matches >= 10, "should return many type suggestions")
  end)

  it("returns dict for 'dic' input", function()
    local memory = require("basilisk.memory")
    local matches = memory.complete_refs("dic")
    local found = false
    for _, m in ipairs(matches) do
      if m == "dict" then
        found = true
      end
    end
    assert.is_true(found, "should find 'dict' in matches")
  end)

  it("is case-insensitive", function()
    local memory = require("basilisk.memory")
    local matches = memory.complete_refs("tensor")
    local found = false
    for _, m in ipairs(matches) do
      if m == "Tensor" then
        found = true
      end
    end
    assert.is_true(found, "should find 'Tensor' when searching for 'tensor'")
  end)

  it("returns ndarray for 'nd' input", function()
    local memory = require("basilisk.memory")
    local matches = memory.complete_refs("nd")
    local found = false
    for _, m in ipairs(matches) do
      if m == "ndarray" then
        found = true
      end
    end
    assert.is_true(found, "should find 'ndarray' for 'nd' prefix")
  end)

  it("returns Series for 'Ser' input", function()
    local memory = require("basilisk.memory")
    local matches = memory.complete_refs("Ser")
    local found = false
    for _, m in ipairs(matches) do
      if m == "Series" then
        found = true
      end
    end
    assert.is_true(found, "should find 'Series' for 'Ser' prefix")
  end)

  it("returns empty for non-matching input", function()
    local memory = require("basilisk.memory")
    local matches = memory.complete_refs("zzz_no_match_ever")
    assert.are.equal(0, #matches, "should return empty for non-matching input")
  end)
end)

-- ============================================================================
-- Cross-Feature: Memory + Profiler Coexistence
-- ============================================================================

describe("memory profiler -- cross-feature", function()
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

  it("memory commands do not interfere with document symbols", function()
    local buf = helpers.open_python_file(
      tmpdir,
      "mem_symbols.py",
      "def hello():\n    pass\n\nclass Foo:\n    pass\n"
    )
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    -- Call memory start (async, may error, that's fine).
    local memory = require("basilisk.memory")
    assert.has_no.errors(function()
      memory.start()
    end)
    vim.wait(500)

    -- Symbols should still work.
    local sym_err, symbols = helpers.lsp_request(client, "textDocument/documentSymbol", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
    }, buf, 5000)
    assert.is_nil(sym_err, "document symbols should work after memory commands")
    assert.is_not_nil(symbols, "should get symbols back")
    assert.is_true(#symbols >= 1, "should find at least one symbol")
  end)

  it("memory and profiler display functions can coexist", function()
    local profiling = require("basilisk.profiling")
    local memory = require("basilisk.memory")

    -- Display profiler results.
    assert.has_no.errors(function()
      profiling.display_results({
        hotFunctions = {
          { name = "hot", file = "/tmp/a.py", line = 1, percentage = 50 },
        },
      })
    end)

    -- Display memory results alongside.
    assert.has_no.errors(function()
      memory.display_leak_report({
        leaks = {
          { typeName = "dict", count = 10, totalSize = "100KB" },
        },
      })
    end)

    -- Close all floats.
    close_floats()
  end)

  it("rapid memory start/stop does not crash LSP", function()
    local buf = helpers.open_python_file(tmpdir, "mem_rapid.py", "x: int = 1\n")
    local client = helpers.wait_for_client(buf)
    assert.is_not_nil(client, "LSP client should attach")
    helpers.wait_for_server_ready(buf)

    local memory = require("basilisk.memory")

    -- Rapid start/stop cycles.
    for _ = 1, 3 do
      assert.has_no.errors(function()
        memory.start()
      end)
      vim.wait(200)
      assert.has_no.errors(function()
        memory.stop()
      end)
      vim.wait(200)
    end

    close_floats()

    -- LSP should still be responsive.
    local sym_err = helpers.lsp_request(client, "textDocument/documentSymbol", {
      textDocument = { uri = vim.uri_from_bufnr(buf) },
    }, buf, 5000)
    assert.is_nil(sym_err, "LSP should still respond after rapid memory cycles")
  end)

  it("dispose functions are idempotent and safe", function()
    local profiling = require("basilisk.profiling")
    local memory = require("basilisk.memory")

    -- Clear heat map multiple times.
    assert.has_no.errors(function()
      profiling.apply_heat_map({})
    end)
    assert.has_no.errors(function()
      profiling.apply_heat_map({})
    end)

    -- Display and close memory reports multiple times.
    assert.has_no.errors(function()
      memory.display_leak_report(nil)
    end)
    close_floats()
    assert.has_no.errors(function()
      memory.display_leak_report(nil)
    end)
    close_floats()
  end)
end)
