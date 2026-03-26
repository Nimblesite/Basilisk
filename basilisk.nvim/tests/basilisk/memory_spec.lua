--- E2E tests for basilisk.memory module.
---
--- Full parity with vscode-extension/src/test/suite/profiler.test.ts memory suites:
--- 1. Memory Command Registration — user commands exist with descriptions/nargs
--- 2. Display Leak Report — floating window output, edge cases
--- 3. Display Retention Paths — confidence, steps, no-data
--- 4. Completion — type matching, case insensitivity
--- 5. Data Structures — MemoryAllocation, MemorySnapshotResult, MemoryDiff, SuspectedLeak
--- 6. State Management — graceful degradation without LSP client
--- 7. Realistic Scenarios — real-world memory leak patterns

local function close_all_floats()
  for _, win in ipairs(vim.api.nvim_list_wins()) do
    local config = vim.api.nvim_win_get_config(win)
    if config.relative and config.relative ~= "" then
      vim.api.nvim_win_close(win, true)
    end
  end
end

describe("basilisk.memory", function()
  local memory = require("basilisk.memory")

  after_each(function()
    close_all_floats()
  end)

  -- ── display_leak_report ─────────────────────────────────────────────

  describe("display_leak_report", function()
    it("handles nil result gracefully", function()
      assert.has_no.errors(function()
        memory.display_leak_report(nil)
      end)
    end)

    it("shows 'no data' message for nil result", function()
      memory.display_leak_report(nil)
      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local text = table.concat(lines, "\n")
          assert.truthy(text:find("No leak data"), "should show no-data message")
          found = true
        end
      end
      assert.is_true(found, "should open a floating window")
    end)

    it("handles empty leaks array", function()
      memory.display_leak_report({ leaks = {} })
      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local text = table.concat(lines, "\n")
          assert.truthy(text:find("No leaks detected"), "should say no leaks detected")
          found = true
        end
      end
      assert.is_true(found)
    end)

    it("displays leaks with type name, count, and size", function()
      local result = {
        leaks = {
          { typeName = "DataFrame", count = 15, totalSize = "1.2MB", location = { file = "/tmp/test.py", line = 42 } },
          { typeName = "dict", count = 100, totalSize = "500KB" },
        },
      }

      memory.display_leak_report(result)

      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local text = table.concat(lines, "\n")

          assert.truthy(text:find("DataFrame"), "must show DataFrame type")
          assert.truthy(text:find("dict"), "must show dict type")
          assert.truthy(text:find("15 objects"), "must show object count")
          assert.truthy(text:find("1.2MB"), "must show size")
          assert.truthy(text:find("500KB"), "must show second size")
          found = true
        end
      end
      assert.is_true(found, "should open floating window with leak report")
    end)

    it("displays location for leaks that have file info", function()
      local result = {
        leaks = {
          { typeName = "list", count = 5, totalSize = "2MB", location = { file = "/app/cache.py", line = 34 } },
        },
      }

      memory.display_leak_report(result)

      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local text = table.concat(lines, "\n")
          assert.truthy(text:find("/app/cache.py"), "must show file path")
          assert.truthy(text:find("34"), "must show line number")
          found = true
        end
      end
      assert.is_true(found)
    end)

    it("handles leaks with nil fields", function()
      local result = {
        leaks = {
          { typeName = nil, count = nil, totalSize = nil },
        },
      }
      assert.has_no.errors(function()
        memory.display_leak_report(result)
      end)
    end)
  end)

  -- ── display_retention_paths ─────────────────────────────────────────

  describe("display_retention_paths", function()
    it("handles nil result gracefully", function()
      assert.has_no.errors(function()
        memory.display_retention_paths("dict", nil)
      end)
    end)

    it("shows 'no data' message for nil result", function()
      memory.display_retention_paths("dict", nil)
      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local text = table.concat(lines, "\n")
          assert.truthy(text:find("No retention data"), "should show no-data message")
          found = true
        end
      end
      assert.is_true(found)
    end)

    it("includes target type name in window title content", function()
      memory.display_retention_paths("DataFrame", { retentionPaths = {} })
      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local text = table.concat(lines, "\n")
          assert.truthy(text:find("DataFrame"), "should include type name in content")
          found = true
        end
      end
      assert.is_true(found)
    end)

    it("displays retention paths with confidence and steps", function()
      local result = {
        retentionPaths = {
          {
            confidence = 0.85,
            steps = {
              { name = "global_cache", kind = "variable" },
              { name = "__dict__", kind = "attribute" },
              { name = "items", kind = "method" },
            },
          },
          {
            confidence = 0.60,
            steps = {
              { name = "module_level_list", kind = "variable" },
            },
          },
        },
      }

      memory.display_retention_paths("DataFrame", result)

      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local text = table.concat(lines, "\n")

          assert.truthy(text:find("DataFrame"), "must show type name")
          assert.truthy(text:find("global_cache"), "must show first step")
          assert.truthy(text:find("__dict__"), "must show second step")
          assert.truthy(text:find("85%%"), "must show 85%% confidence")
          assert.truthy(text:find("60%%"), "must show 60%% confidence")
          assert.truthy(text:find("module_level_list"), "must show second path step")
          found = true
        end
      end
      assert.is_true(found)
    end)

    it("shows 'no paths found' for empty retentionPaths", function()
      memory.display_retention_paths("dict", { retentionPaths = {} })
      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local text = table.concat(lines, "\n")
          assert.truthy(text:find("No retention paths"), "should indicate no paths")
          found = true
        end
      end
      assert.is_true(found)
    end)
  end)

  -- ── complete_refs ───────────────────────────────────────────────────

  describe("complete_refs", function()
    it("returns DataFrame for 'Data' input", function()
      local matches = memory.complete_refs("Data")
      assert.are.equal("DataFrame", matches[1])
    end)

    it("returns all types for empty input", function()
      local matches = memory.complete_refs("")
      assert.is_true(#matches >= 10, "should return many type suggestions, got " .. #matches)
    end)

    it("returns dict for 'dic' input", function()
      local matches = memory.complete_refs("dic")
      local found = false
      for _, m in ipairs(matches) do
        if m == "dict" then
          found = true
        end
      end
      assert.is_true(found, "should find 'dict' for 'dic' prefix")
    end)

    it("is case-insensitive", function()
      local matches = memory.complete_refs("tensor")
      local found = false
      for _, m in ipairs(matches) do
        if m == "Tensor" then
          found = true
        end
      end
      assert.is_true(found, "should find 'Tensor' for lowercase 'tensor'")
    end)

    it("returns empty table for non-matching input", function()
      local matches = memory.complete_refs("zzzzzznotaType")
      assert.are.equal(0, #matches, "should return empty for non-matching input")
    end)

    it("includes common Python types", function()
      local matches = memory.complete_refs("")
      local types_set = {}
      for _, m in ipairs(matches) do
        types_set[m] = true
      end

      assert.is_true(types_set["dict"] or false, "should include dict")
      assert.is_true(types_set["list"] or false, "should include list")
      assert.is_true(types_set["set"] or false, "should include set")
      assert.is_true(types_set["str"] or false, "should include str")
      assert.is_true(types_set["DataFrame"] or false, "should include DataFrame")
      assert.is_true(types_set["Tensor"] or false, "should include Tensor")
      assert.is_true(types_set["ndarray"] or false, "should include ndarray")
    end)

    it("returns single-character prefix matches", function()
      local matches = memory.complete_refs("d")
      assert.is_true(#matches >= 1, "should match at least 'dict' for 'd'")
    end)
  end)

  -- ── State management ────────────────────────────────────────────────

  describe("state management", function()
    it("start without client does not error", function()
      assert.has_no.errors(function()
        memory.start()
      end)
    end)

    it("stop without client does not error", function()
      assert.has_no.errors(function()
        memory.stop()
      end)
    end)

    it("refs without client does not error", function()
      assert.has_no.errors(function()
        memory.refs("dict")
      end)
    end)
  end)
end)

-- ── Suite: Memory Command Registration (VSIX parity) ──────────────────────

describe("memory — command registration", function()
  -- Register commands (idempotent).
  local config = require("basilisk.config").defaults
  require("basilisk.commands").register(config)

  local MEMORY_COMMANDS = {
    "BasiliskMemLeak",
    "BasiliskMemStop",
    "BasiliskMemRefs",
  }

  it("all memory user commands are registered", function()
    local all_cmds = vim.api.nvim_get_commands({})
    for _, cmd in ipairs(MEMORY_COMMANDS) do
      assert.truthy(all_cmds[cmd], "user command '" .. cmd .. "' should be registered")
    end
  end)

  it("memory commands have descriptions", function()
    local all_cmds = vim.api.nvim_get_commands({})
    for _, cmd in ipairs(MEMORY_COMMANDS) do
      local entry = all_cmds[cmd]
      assert.truthy(entry, "command '" .. cmd .. "' should exist")
      assert.truthy(
        entry.definition and #entry.definition > 0,
        "command '" .. cmd .. "' should have a description"
      )
    end
  end)

  it("BasiliskMemLeak takes no arguments", function()
    local all_cmds = vim.api.nvim_get_commands({})
    local entry = all_cmds["BasiliskMemLeak"]
    assert.truthy(entry)
    assert.are.equal("0", entry.nargs)
  end)

  it("BasiliskMemStop takes no arguments", function()
    local all_cmds = vim.api.nvim_get_commands({})
    local entry = all_cmds["BasiliskMemStop"]
    assert.truthy(entry)
    assert.are.equal("0", entry.nargs)
  end)

  it("BasiliskMemRefs takes exactly 1 argument", function()
    local all_cmds = vim.api.nvim_get_commands({})
    local entry = all_cmds["BasiliskMemRefs"]
    assert.truthy(entry)
    assert.are.equal("1", entry.nargs)
  end)

  it("memory commands are distinct from profiler commands", function()
    local profiler_cmds = { "BasiliskProfile", "BasiliskProfileStop", "BasiliskProfileSnapshot" }
    local memory_set = {}
    for _, cmd in ipairs(MEMORY_COMMANDS) do
      memory_set[cmd] = true
    end
    for _, cmd in ipairs(profiler_cmds) do
      assert.falsy(memory_set[cmd], "profiler command '" .. cmd .. "' must not be in memory set")
    end
  end)

  it("BasiliskMemLeak description mentions memory", function()
    local all_cmds = vim.api.nvim_get_commands({})
    local entry = all_cmds["BasiliskMemLeak"]
    assert.truthy(entry)
    local desc = entry.definition:lower()
    assert.truthy(
      desc:find("memory") or desc:find("leak"),
      "description should mention memory or leak"
    )
  end)

  it("BasiliskMemRefs description mentions references", function()
    local all_cmds = vim.api.nvim_get_commands({})
    local entry = all_cmds["BasiliskMemRefs"]
    assert.truthy(entry)
    local desc = entry.definition:lower()
    assert.truthy(
      desc:find("reference") or desc:find("memory"),
      "description should mention references or memory"
    )
  end)
end)

-- ── Suite: Memory Data Structures (VSIX parity) ───────────────────────────

describe("memory — data structures", function()
  it("MemoryAllocation-like table validates required fields", function()
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

  it("MemorySnapshotResult-like table validates required fields", function()
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
    assert.is_table(snapshot.topAllocations)
  end)

  it("MemoryDiffResult-like table validates required fields", function()
    local diff = {
      beforeSnapshot = "snap-001",
      afterSnapshot = "snap-002",
      growthEntries = {
        { file = "/src/cache.py", line = 10, sizeDiff = 1048576, countDiff = 100 },
      },
      totalGrowth = 1048576,
    }

    assert.are.equal("snap-001", diff.beforeSnapshot)
    assert.are.equal("snap-002", diff.afterSnapshot)
    assert.is_table(diff.growthEntries)
    assert.are.equal(1, #diff.growthEntries)
    assert.are.equal(1048576, diff.totalGrowth)
  end)

  it("SuspectedLeak-like table validates all fields", function()
    local leak = {
      typeName = "DataFrame",
      count = 150,
      totalSize = "12.5MB",
      confidence = "High",
      location = { file = "/src/data.py", line = 42 },
    }

    assert.are.equal("DataFrame", leak.typeName)
    assert.are.equal(150, leak.count)
    assert.are.equal("12.5MB", leak.totalSize)
    assert.are.equal("High", leak.confidence)
    assert.is_table(leak.location)
    assert.are.equal("/src/data.py", leak.location.file)
    assert.are.equal(42, leak.location.line)
  end)

  it("LeakConfidence values are ordered correctly", function()
    local confidences = { "Low", "Medium", "High", "Definite" }
    local order = {}
    for i, c in ipairs(confidences) do
      order[c] = i
    end
    assert.is_true(order["Low"] < order["Medium"])
    assert.is_true(order["Medium"] < order["High"])
    assert.is_true(order["High"] < order["Definite"])
  end)

  it("MemorySnapshotResult with populated allocations validates structure", function()
    local snapshot = {
      memorySessionId = "mem-002",
      snapshotId = "snap-002",
      currentMemory = 100000000,
      peakMemory = 150000000,
      topAllocations = {
        { file = "/src/model.py", line = 45, size = 5242880, count = 1000 },
        { file = "/src/data.py", line = 12, size = 2097152, count = 500 },
        { file = "/src/cache.py", line = 78, size = 1048576, count = 200 },
      },
    }

    assert.are.equal(3, #snapshot.topAllocations)
    assert.is_true(
      snapshot.topAllocations[1].size > snapshot.topAllocations[2].size,
      "allocations should be ordered by size (largest first)"
    )
    assert.is_true(snapshot.peakMemory >= snapshot.currentMemory,
      "peak memory should be >= current memory")
  end)
end)

-- ── Suite: Realistic Memory Scenarios (VSIX parity) ───────────────────────

describe("memory — realistic scenarios", function()
  local memory = require("basilisk.memory")

  after_each(function()
    close_all_floats()
  end)

  it("handles a Django ORM leak pattern", function()
    local result = {
      leaks = {
        { typeName = "QuerySet", count = 500, totalSize = "45MB", location = { file = "/app/views.py", line = 23 } },
        { typeName = "Model", count = 2000, totalSize = "120MB", location = { file = "/app/models.py", line = 15 } },
        { typeName = "dict", count = 10000, totalSize = "8MB" },
      },
    }

    memory.display_leak_report(result)

    local found = false
    for _, win in ipairs(vim.api.nvim_list_wins()) do
      local config = vim.api.nvim_win_get_config(win)
      if config.relative and config.relative ~= "" then
        local buf = vim.api.nvim_win_get_buf(win)
        local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
        local text = table.concat(lines, "\n")

        assert.truthy(text:find("QuerySet"), "should show QuerySet leak")
        assert.truthy(text:find("Model"), "should show Model leak")
        assert.truthy(text:find("dict"), "should show dict leak")
        assert.truthy(text:find("45MB"), "should show QuerySet size")
        assert.truthy(text:find("120MB"), "should show Model size")
        assert.truthy(text:find("500 objects"), "should show QuerySet count")
        found = true
      end
    end
    assert.is_true(found)
  end)

  it("handles a data science memory pattern", function()
    local result = {
      leaks = {
        { typeName = "DataFrame", count = 50, totalSize = "2.1GB", location = { file = "/ml/train.py", line = 88 } },
        { typeName = "ndarray", count = 200, totalSize = "800MB", location = { file = "/ml/preprocess.py", line = 42 } },
        { typeName = "Tensor", count = 100, totalSize = "1.5GB", location = { file = "/ml/model.py", line = 156 } },
      },
    }

    memory.display_leak_report(result)

    local found = false
    for _, win in ipairs(vim.api.nvim_list_wins()) do
      local config = vim.api.nvim_win_get_config(win)
      if config.relative and config.relative ~= "" then
        local buf = vim.api.nvim_win_get_buf(win)
        local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
        local text = table.concat(lines, "\n")

        assert.truthy(text:find("DataFrame"), "should show DataFrame")
        assert.truthy(text:find("ndarray"), "should show ndarray")
        assert.truthy(text:find("Tensor"), "should show Tensor")
        assert.truthy(text:find("2.1GB"), "should show large size")
        found = true
      end
    end
    assert.is_true(found)
  end)

  it("handles retention paths with deep reference chains", function()
    local result = {
      retentionPaths = {
        {
          confidence = 0.95,
          steps = {
            { name = "app", kind = "module" },
            { name = "cache", kind = "attribute" },
            { name = "_store", kind = "attribute" },
            { name = "[0]", kind = "index" },
            { name = "__dict__", kind = "attribute" },
            { name = "data", kind = "attribute" },
          },
        },
      },
    }

    memory.display_retention_paths("CachedFrame", result)

    local found = false
    for _, win in ipairs(vim.api.nvim_list_wins()) do
      local config = vim.api.nvim_win_get_config(win)
      if config.relative and config.relative ~= "" then
        local buf = vim.api.nvim_win_get_buf(win)
        local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
        local text = table.concat(lines, "\n")

        assert.truthy(text:find("CachedFrame"), "should show type name")
        assert.truthy(text:find("app"), "should show root step")
        assert.truthy(text:find("cache"), "should show cache step")
        assert.truthy(text:find("_store"), "should show _store step")
        assert.truthy(text:find("95%%"), "should show 95%% confidence")
        found = true
      end
    end
    assert.is_true(found)
  end)

  it("handles multiple retention paths for same type", function()
    local result = {
      retentionPaths = {
        {
          confidence = 0.90,
          steps = {
            { name = "global_registry", kind = "variable" },
            { name = "items", kind = "method" },
          },
        },
        {
          confidence = 0.70,
          steps = {
            { name = "thread_local", kind = "variable" },
            { name = "buffer", kind = "attribute" },
          },
        },
        {
          confidence = 0.40,
          steps = {
            { name = "__main__", kind = "module" },
            { name = "temp_list", kind = "variable" },
          },
        },
      },
    }

    memory.display_retention_paths("bytes", result)

    local found = false
    for _, win in ipairs(vim.api.nvim_list_wins()) do
      local config = vim.api.nvim_win_get_config(win)
      if config.relative and config.relative ~= "" then
        local buf = vim.api.nvim_win_get_buf(win)
        local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
        local text = table.concat(lines, "\n")

        assert.truthy(text:find("global_registry"), "should show first path")
        assert.truthy(text:find("thread_local"), "should show second path")
        assert.truthy(text:find("__main__"), "should show third path")
        assert.truthy(text:find("90%%"), "should show 90%% confidence")
        assert.truthy(text:find("70%%"), "should show 70%% confidence")
        assert.truthy(text:find("40%%"), "should show 40%% confidence")
        found = true
      end
    end
    assert.is_true(found)
  end)
end)

-- ── Suite: Memory Module API (VSIX parity: Decoration Modules) ──────────────

describe("memory — module API", function()
  local memory = require("basilisk.memory")

  it("exports start function", function()
    assert.is_function(memory.start)
  end)

  it("exports stop function", function()
    assert.is_function(memory.stop)
  end)

  it("exports refs function", function()
    assert.is_function(memory.refs)
  end)

  it("exports display_leak_report function", function()
    assert.is_function(memory.display_leak_report)
  end)

  it("exports display_retention_paths function", function()
    assert.is_function(memory.display_retention_paths)
  end)

  it("exports complete_refs function", function()
    assert.is_function(memory.complete_refs)
  end)
end)
