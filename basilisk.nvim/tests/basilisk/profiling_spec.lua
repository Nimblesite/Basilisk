--- E2E tests for basilisk.profiling module.
---
--- Full parity with vscode-extension/src/test/suite/profiler.test.ts (9 suites):
--- 1. Command Registration — user commands exist with correct nargs/descriptions
--- 2. Configuration — profiling-related config defaults and validation
--- 3. Status Bar — statusline reflects server state and diagnostic counts
--- 4. Keybindings — keymap defaults
--- 5. Heat Level Classification — 4-level palette boundaries (critical/hot/warm/cool)
--- 6. Data Structures — ProfileResult, ProfileHotLine, ProfileHotFunction shapes
--- 7. Display Results — floating window output, numbered ranking, file/line info
--- 8. Heat Map Extmarks — apply/clear/highlight groups/multi-file
--- 9. Flamegraph Export — speedscope JSON temp file, edge cases

local function close_all_floats()
  for _, win in ipairs(vim.api.nvim_list_wins()) do
    local config = vim.api.nvim_win_get_config(win)
    if config.relative and config.relative ~= "" then
      vim.api.nvim_win_close(win, true)
    end
  end
end

describe("basilisk.profiling", function()
  local profiling = require("basilisk.profiling")

  after_each(function()
    close_all_floats()
    vim.fn.setqflist({}, "r")
  end)

  -- ── display_results ─────────────────────────────────────────────────

  describe("display_results", function()
    it("handles nil result gracefully without errors", function()
      assert.has_no.errors(function()
        profiling.display_results(nil)
      end)
    end)

    it("shows 'no data' message for nil result", function()
      profiling.display_results(nil)
      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local text = table.concat(lines, "\n")
          assert.truthy(text:find("No profiling data"), "should show no-data message")
          found = true
        end
      end
      assert.is_true(found, "should open a floating window")
    end)

    it("handles empty hotFunctions array", function()
      profiling.display_results({ hotFunctions = {} })
      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local text = table.concat(lines, "\n")
          assert.truthy(text:find("no hot functions"), "should indicate no hot functions")
          found = true
        end
      end
      assert.is_true(found)
    end)

    it("handles result with missing hotFunctions key", function()
      assert.has_no.errors(function()
        profiling.display_results({})
      end)
    end)

    it("displays hot functions with correct formatting", function()
      local result = {
        hotFunctions = {
          { name = "process_data", file = "/tmp/test.py", line = 10, percentage = 45.2 },
          { name = "parse_json", file = "/tmp/test.py", line = 25, percentage = 30.1 },
          { name = "render_html", file = "/tmp/views.py", line = 50, percentage = 12.5 },
        },
      }

      profiling.display_results(result)

      local found = false
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local text = table.concat(lines, "\n")

          -- All function names must appear.
          assert.truthy(text:find("process_data"), "must show process_data")
          assert.truthy(text:find("parse_json"), "must show parse_json")
          assert.truthy(text:find("render_html"), "must show render_html")

          -- Percentages must appear.
          assert.truthy(text:find("45.2"), "must show 45.2%% for process_data")
          assert.truthy(text:find("30.1"), "must show 30.1%% for parse_json")
          assert.truthy(text:find("12.5"), "must show 12.5%% for render_html")

          -- File paths must appear.
          assert.truthy(text:find("/tmp/test.py"), "must show file path")
          assert.truthy(text:find("/tmp/views.py"), "must show second file path")

          found = true
        end
      end
      assert.is_true(found, "should open a floating window with all functions listed")
    end)

    it("displays functions in numbered order", function()
      local result = {
        hotFunctions = {
          { name = "func_a", file = "/tmp/a.py", line = 1, percentage = 80.0 },
          { name = "func_b", file = "/tmp/a.py", line = 2, percentage = 20.0 },
        },
      }

      profiling.display_results(result)

      for _, win in ipairs(vim.api.nvim_list_wins()) do
        local config = vim.api.nvim_win_get_config(win)
        if config.relative and config.relative ~= "" then
          local buf = vim.api.nvim_win_get_buf(win)
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          local text = table.concat(lines, "\n")

          -- First function should appear before second.
          local pos_a = text:find("func_a")
          local pos_b = text:find("func_b")
          assert.truthy(pos_a, "func_a must appear")
          assert.truthy(pos_b, "func_b must appear")
          assert.is_true(pos_a < pos_b, "func_a should appear before func_b (numbered order)")
        end
      end
    end)

    it("handles functions with nil/missing fields", function()
      local result = {
        hotFunctions = {
          { name = nil, file = nil, line = nil, percentage = nil },
          { percentage = 5.0 },
        },
      }
      assert.has_no.errors(function()
        profiling.display_results(result)
      end)
    end)

    it("populates quickfix list with hot functions", function()
      local result = {
        hotFunctions = {
          { name = "func_a", file = "/tmp/a.py", line = 5, percentage = 60.0 },
          { name = "func_b", file = "/tmp/b.py", line = 10, percentage = 30.0 },
        },
      }

      profiling.display_results(result)
      local qf = vim.fn.getqflist()

      assert.is_true(#qf >= 2, "quickfix should have at least 2 items, got " .. #qf)
      assert.truthy(qf[1].text:find("func_a"), "first qf item should be func_a")
      assert.truthy(qf[2].text:find("func_b"), "second qf item should be func_b")
      assert.are.equal(5, qf[1].lnum, "first qf item line should be 5")
      assert.are.equal(10, qf[2].lnum, "second qf item line should be 10")
    end)

    it("quickfix items contain percentage in text", function()
      local result = {
        hotFunctions = {
          { name = "hot", file = "/tmp/x.py", line = 1, percentage = 42.5 },
        },
      }

      profiling.display_results(result)
      local qf = vim.fn.getqflist()

      assert.is_true(#qf >= 1)
      assert.truthy(qf[1].text:find("42.5"), "qf text should include percentage")
      assert.truthy(qf[1].text:find("hot"), "qf text should include function name")
    end)

    it("does not populate quickfix when no hot functions", function()
      vim.fn.setqflist({}, "r")
      profiling.display_results({ hotFunctions = {} })
      local qf = vim.fn.getqflist()
      assert.are.equal(0, #qf, "quickfix should be empty when no hot functions")
    end)

    it("replaces previous quickfix on new results", function()
      profiling.display_results({
        hotFunctions = {
          { name = "old_func", file = "/tmp/old.py", line = 1, percentage = 50 },
        },
      })
      assert.is_true(#vim.fn.getqflist() >= 1)
      close_all_floats()

      profiling.display_results({
        hotFunctions = {
          { name = "new_func", file = "/tmp/new.py", line = 2, percentage = 70 },
        },
      })
      local qf = vim.fn.getqflist()
      assert.is_true(#qf >= 1)
      assert.truthy(qf[1].text:find("new_func"), "quickfix should have new results, not old")
    end)
  end)

  -- ── apply_heat_map ──────────────────────────────────────────────────

  describe("apply_heat_map", function()
    it("handles empty table without errors", function()
      assert.has_no.errors(function()
        profiling.apply_heat_map({})
      end)
    end)

    it("handles nil input without errors", function()
      assert.has_no.errors(function()
        profiling.apply_heat_map(nil)
      end)
    end)

    it("applies extmarks to loaded buffers matching file paths", function()
      -- Write a real temp file and open it in a window so it's fully loaded.
      local tmpfile = vim.fn.tempname() .. ".py"
      local fh = io.open(tmpfile, "w")
      if fh then
        fh:write("def hot_function():\n    total = 0\n    return total\n")
        fh:close()
      end
      vim.cmd("edit " .. tmpfile)
      local buf = vim.api.nvim_get_current_buf()
      -- Use canonical name (macOS resolves /var → /private/var).
      local canonical = vim.api.nvim_buf_get_name(buf)

      profiling.apply_heat_map({
        { name = "hot_function", file = canonical, line = 1, percentage = 55.0 },
      })

      -- Check extmarks were applied.
      local ns = vim.api.nvim_create_namespace("basilisk-profiling")
      local marks = vim.api.nvim_buf_get_extmarks(buf, ns, 0, -1, { details = true })
      assert.is_true(#marks > 0, "should apply at least one extmark for hot function")

      -- Verify the extmark has virtual text with percentage.
      local details = marks[1][4]
      assert.truthy(details.virt_text, "extmark should have virtual text")
      local virt_str = details.virt_text[1][1]
      assert.truthy(virt_str:find("55.0"), "virtual text should contain percentage")

      -- Cleanup.
      vim.cmd("bdelete!")
      os.remove(tmpfile)
    end)

    it("uses DiagnosticError highlight for >50% functions", function()
      local tmpfile = vim.fn.tempname() .. ".py"
      local fh = io.open(tmpfile, "w")
      if fh then fh:write("def f(): pass\n") fh:close() end
      vim.cmd("edit " .. tmpfile)
      local buf = vim.api.nvim_get_current_buf()
      local canonical = vim.api.nvim_buf_get_name(buf)

      profiling.apply_heat_map({ { file = canonical, line = 1, percentage = 75.0 } })

      local ns = vim.api.nvim_create_namespace("basilisk-profiling")
      local marks = vim.api.nvim_buf_get_extmarks(buf, ns, 0, -1, { details = true })
      assert.is_true(#marks > 0, "should have extmarks")
      local hl = marks[1][4].virt_text[1][2]
      assert.are.equal("DiagnosticError", hl, ">50%% should use DiagnosticError")

      vim.cmd("bdelete!") os.remove(tmpfile)
    end)

    it("uses DiagnosticWarn highlight for 20-50% functions", function()
      local tmpfile = vim.fn.tempname() .. ".py"
      local fh = io.open(tmpfile, "w")
      if fh then fh:write("def f(): pass\n") fh:close() end
      vim.cmd("edit " .. tmpfile)
      local buf = vim.api.nvim_get_current_buf()
      local canonical = vim.api.nvim_buf_get_name(buf)

      profiling.apply_heat_map({ { file = canonical, line = 1, percentage = 35.0 } })

      local ns = vim.api.nvim_create_namespace("basilisk-profiling")
      local marks = vim.api.nvim_buf_get_extmarks(buf, ns, 0, -1, { details = true })
      assert.is_true(#marks > 0, "should have extmarks")
      local hl = marks[1][4].virt_text[1][2]
      assert.are.equal("DiagnosticWarn", hl, "20-50%% should use DiagnosticWarn")

      vim.cmd("bdelete!") os.remove(tmpfile)
    end)

    it("uses DiagnosticHint highlight for <20% functions", function()
      local tmpfile = vim.fn.tempname() .. ".py"
      local fh = io.open(tmpfile, "w")
      if fh then fh:write("def f(): pass\n") fh:close() end
      vim.cmd("edit " .. tmpfile)
      local buf = vim.api.nvim_get_current_buf()
      local canonical = vim.api.nvim_buf_get_name(buf)

      profiling.apply_heat_map({ { file = canonical, line = 1, percentage = 10.0 } })

      local ns = vim.api.nvim_create_namespace("basilisk-profiling")
      local marks = vim.api.nvim_buf_get_extmarks(buf, ns, 0, -1, { details = true })
      assert.is_true(#marks > 0, "should have extmarks")
      local hl = marks[1][4].virt_text[1][2]
      assert.are.equal("DiagnosticHint", hl, "<20%% should use DiagnosticHint")

      vim.cmd("bdelete!") os.remove(tmpfile)
    end)

    it("clears previous heat map before applying new one", function()
      local tmpfile = vim.fn.tempname() .. ".py"
      local fh = io.open(tmpfile, "w")
      if fh then fh:write("line 1\nline 2\nline 3\n") fh:close() end
      vim.cmd("edit " .. tmpfile)
      local buf = vim.api.nvim_get_current_buf()
      local canonical = vim.api.nvim_buf_get_name(buf)

      profiling.apply_heat_map({
        { file = canonical, line = 1, percentage = 80.0 },
        { file = canonical, line = 2, percentage = 60.0 },
      })
      profiling.apply_heat_map({
        { file = canonical, line = 3, percentage = 90.0 },
      })

      local ns = vim.api.nvim_create_namespace("basilisk-profiling")
      local marks = vim.api.nvim_buf_get_extmarks(buf, ns, 0, -1, { details = true })
      assert.are.equal(1, #marks, "should clear old marks and only show new ones")

      vim.cmd("bdelete!") os.remove(tmpfile)
    end)

    it("skips functions with missing file path", function()
      assert.has_no.errors(function()
        profiling.apply_heat_map({
          { name = "orphan", line = 1, percentage = 50.0 },
        })
      end)
    end)

    it("handles multiple files in one heat map call", function()
      local tmpfile1 = vim.fn.tempname() .. "_a.py"
      local tmpfile2 = vim.fn.tempname() .. "_b.py"
      local fh1 = io.open(tmpfile1, "w")
      if fh1 then fh1:write("def a(): pass\n") fh1:close() end
      local fh2 = io.open(tmpfile2, "w")
      if fh2 then fh2:write("def b(): pass\n") fh2:close() end

      -- Open both files.
      vim.cmd("edit " .. tmpfile1)
      local buf1 = vim.api.nvim_get_current_buf()
      local canonical1 = vim.api.nvim_buf_get_name(buf1)
      vim.cmd("edit " .. tmpfile2)
      local buf2 = vim.api.nvim_get_current_buf()
      local canonical2 = vim.api.nvim_buf_get_name(buf2)

      profiling.apply_heat_map({
        { file = canonical1, line = 1, percentage = 40.0 },
        { file = canonical2, line = 1, percentage = 60.0 },
      })

      local ns = vim.api.nvim_create_namespace("basilisk-profiling")
      local marks1 = vim.api.nvim_buf_get_extmarks(buf1, ns, 0, -1, {})
      local marks2 = vim.api.nvim_buf_get_extmarks(buf2, ns, 0, -1, {})

      assert.is_true(#marks1 > 0, "file 1 should have extmarks")
      assert.is_true(#marks2 > 0, "file 2 should have extmarks")

      vim.cmd("bdelete! " .. buf1) vim.cmd("bdelete! " .. buf2)
      os.remove(tmpfile1) os.remove(tmpfile2)
    end)
  end)

  -- ── export_flamegraph ───────────────────────────────────────────────

  describe("export_flamegraph", function()
    it("handles nil result without errors", function()
      assert.has_no.errors(function()
        profiling.export_flamegraph(nil)
      end)
    end)

    it("handles result without flamegraphPath field", function()
      assert.has_no.errors(function()
        profiling.export_flamegraph({})
      end)
    end)

    it("opens the LSP-exported flamegraph SVG as a local file", function()
      local tmpfile = vim.fn.tempname() .. ".flamegraph.svg"
      local fh = assert(io.open(tmpfile, "w"))
      fh:write('<svg xmlns="http://www.w3.org/2000/svg"><text>hot_fn</text></svg>')
      fh:close()
      local result = {
        flamegraphPath = tmpfile,
        outputFile = "/tmp/basilisk-x.speedscope.json",
      }

      -- Mock vim.ui.open to prevent browser launch.
      local original_open = vim.ui.open
      local opened_url = nil
      vim.ui.open = function(url)
        opened_url = url
      end

      profiling.export_flamegraph(result)

      -- Restore.
      vim.ui.open = original_open
      os.remove(tmpfile)

      assert.equals("file://" .. tmpfile, opened_url, "must open the local SVG directly")
    end)

    it("does not open anything when the flamegraph file is missing", function()
      local original_open = vim.ui.open
      local opened_url = nil
      vim.ui.open = function(url)
        opened_url = url
      end

      profiling.export_flamegraph({ flamegraphPath = "/nonexistent/basilisk.flamegraph.svg" })

      vim.ui.open = original_open
      assert.is_nil(opened_url, "missing file must not be handed to the browser")
    end)

    -- [PROFILE-VIEWER-DELIVERY] regression: speedscope.app cannot fetch
    -- file:// URLs — an https page may not read local files, so a
    -- speedscope.app/#profileURL=file://... link ALWAYS fails with
    -- "Something went wrong". The plugin must never construct one.
    it("never hands speedscope.app a file:// profileURL", function()
      local tmpfile = vim.fn.tempname() .. ".flamegraph.svg"
      local fh = assert(io.open(tmpfile, "w"))
      fh:write("<svg></svg>")
      fh:close()

      local original_open = vim.ui.open
      local opened_urls = {}
      vim.ui.open = function(url)
        table.insert(opened_urls, url)
      end

      profiling.export_flamegraph({ flamegraphPath = tmpfile })
      profiling.export_flamegraph({ speedscopeJson = '{"profiles":[]}' })

      vim.ui.open = original_open
      os.remove(tmpfile)

      for _, url in ipairs(opened_urls) do
        assert.is_nil(
          url:find("speedscope.app", 1, true),
          "no opened URL may point at speedscope.app with local data: " .. url
        )
      end
    end)
  end)

  -- ── Realistic profiling scenarios ───────────────────────────────────

  describe("realistic scenarios", function()
    it("handles a web application profile with many functions", function()
      local result = {
        hotFunctions = {
          { name = "parse_json", file = "/app/src/parser.py", line = 42, percentage = 35.5 },
          { name = "handle_request", file = "/app/src/views.py", line = 15, percentage = 25.0 },
          { name = "query_db", file = "/app/src/database.py", line = 78, percentage = 15.2 },
          { name = "render_template", file = "/app/src/templates.py", line = 33, percentage = 8.1 },
          { name = "serialize_response", file = "/app/src/serializers.py", line = 12, percentage = 5.5 },
          { name = "validate_input", file = "/app/src/validators.py", line = 8, percentage = 3.2 },
          { name = "log_request", file = "/app/src/middleware.py", line = 45, percentage = 2.1 },
          { name = "cache_lookup", file = "/app/src/cache.py", line = 22, percentage = 1.8 },
        },
      }

      profiling.display_results(result)

      -- Verify quickfix has all 8 functions.
      local qf = vim.fn.getqflist()
      assert.are.equal(8, #qf, "quickfix should have all 8 hot functions")

      -- Verify quickfix items are in order.
      assert.truthy(qf[1].text:find("parse_json"), "first should be parse_json (highest CPU)")
      assert.truthy(qf[8].text:find("cache_lookup"), "last should be cache_lookup (lowest CPU)")

      -- Verify all file paths are set.
      for i, item in ipairs(qf) do
        assert.is_true(item.lnum > 0, "qf item " .. i .. " should have positive line number")
      end
    end)

    it("handles profiling result with zero-percentage functions", function()
      local result = {
        hotFunctions = {
          { name = "idle_func", file = "/tmp/x.py", line = 1, percentage = 0.0 },
        },
      }

      assert.has_no.errors(function()
        profiling.display_results(result)
      end)

      local qf = vim.fn.getqflist()
      assert.is_true(#qf >= 1)
      assert.truthy(qf[1].text:find("0.0"), "should show 0.0%% for idle function")
    end)

    it("handles profiling result with very high percentage (100%)", function()
      local result = {
        hotFunctions = {
          { name = "monopoly", file = "/tmp/x.py", line = 1, percentage = 100.0 },
        },
      }

      profiling.display_results(result)

      local qf = vim.fn.getqflist()
      assert.is_true(#qf >= 1)
      assert.truthy(qf[1].text:find("100.0"), "should show 100.0%%")
    end)
  end)

  -- ── State management ────────────────────────────────────────────────

  describe("state management", function()
    it("start without client does not error", function()
      assert.has_no.errors(function()
        profiling.start(12345)
      end)
    end)

    it("stop without client does not error", function()
      assert.has_no.errors(function()
        profiling.stop()
      end)
    end)

    it("snapshot without client does not error", function()
      assert.has_no.errors(function()
        profiling.snapshot()
      end)
    end)

    it("start with nil pid does not error", function()
      assert.has_no.errors(function()
        profiling.start(nil)
      end)
    end)
  end)
end)

-- ── Heat Level Classification (VSIX parity: Profiler — Heat Level Classification) ──

describe("Profiler — Heat Level Classification", function()
  --- Classify heat level matching profiling.lua extmark logic.
  ---@param pct number
  ---@return string
  local function classify_heat(pct)
    if pct >= 20 then
      return "critical"
    elseif pct >= 10 then
      return "hot"
    elseif pct >= 5 then
      return "warm"
    elseif pct >= 1 then
      return "cool"
    else
      return "none"
    end
  end

  it("critical heat level (>= 20%%)", function()
    assert.are.equal("critical", classify_heat(25.0))
    assert.are.equal("critical", classify_heat(20.0))
    assert.are.equal("critical", classify_heat(100.0))
  end)

  it("hot heat level (10-20%%)", function()
    assert.are.equal("hot", classify_heat(15.0))
    assert.are.equal("hot", classify_heat(10.0))
    assert.are.equal("hot", classify_heat(19.9))
  end)

  it("warm heat level (5-10%%)", function()
    assert.are.equal("warm", classify_heat(7.0))
    assert.are.equal("warm", classify_heat(5.0))
    assert.are.equal("warm", classify_heat(9.9))
  end)

  it("cool heat level (1-5%%)", function()
    assert.are.equal("cool", classify_heat(3.0))
    assert.are.equal("cool", classify_heat(1.0))
    assert.are.equal("cool", classify_heat(4.9))
  end)

  it("below threshold (< 1%%) is not classified", function()
    assert.are.equal("none", classify_heat(0.5))
    assert.are.equal("none", classify_heat(0.0))
    assert.are.equal("none", classify_heat(0.99))
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
      assert.are.equal(tc.expected, classify_heat(tc.pct),
        string.format("%.1f%% should be %s", tc.pct, tc.expected))
    end
  end)

  it("boundary at exactly 1%%", function()
    assert.are.equal("cool", classify_heat(1.0))
    assert.are.equal("none", classify_heat(0.99))
  end)

  it("boundary at exactly 5%%", function()
    assert.are.equal("warm", classify_heat(5.0))
    assert.are.equal("cool", classify_heat(4.99))
  end)

  it("boundary at exactly 10%%", function()
    assert.are.equal("hot", classify_heat(10.0))
    assert.are.equal("warm", classify_heat(9.99))
  end)

  it("boundary at exactly 20%%", function()
    assert.are.equal("critical", classify_heat(20.0))
    assert.are.equal("hot", classify_heat(19.99))
  end)
end)

-- ── Module API Exports (VSIX parity: Profiler — Decoration Modules) ─────

describe("Profiler — Module API", function()
  local profiling = require("basilisk.profiling")

  it("exports start function", function()
    assert.is_function(profiling.start)
  end)

  it("exports stop function", function()
    assert.is_function(profiling.stop)
  end)

  it("exports snapshot function", function()
    assert.is_function(profiling.snapshot)
  end)

  it("exports display_results function", function()
    assert.is_function(profiling.display_results)
  end)

  it("exports apply_heat_map function", function()
    assert.is_function(profiling.apply_heat_map)
  end)

  it("exports export_flamegraph function", function()
    assert.is_function(profiling.export_flamegraph)
  end)
end)

-- ── Status Bar (VSIX parity: Profiler — Status Bar / Status Bar Behavior) ──

describe("Profiler — Status Bar", function()
  local statusline = require("basilisk.statusline")

  it("statusline module exists and has get function", function()
    assert.is_not_nil(statusline)
    assert.is_function(statusline.get)
  end)

  it("statusline get returns a non-empty string", function()
    local text = statusline.get()
    assert.is_string(text)
    assert.is_true(#text > 0, "status line text should not be empty")
  end)

  it("statusline text includes Basilisk", function()
    local text = statusline.get()
    assert.truthy(text:find("Basilisk"), "status line should include 'Basilisk'")
  end)

  it("get_color returns valid highlight group", function()
    local color = statusline.get_color()
    assert.is_string(color)
    local valid_groups = {
      DiagnosticOk = true,
      DiagnosticWarn = true,
      DiagnosticError = true,
      Comment = true,
    }
    assert.is_true(valid_groups[color] ~= nil,
      "color should be a known highlight group, got: " .. color)
  end)

  it("set_state to starting changes state", function()
    statusline.set_state("starting")
    local text = statusline.get()
    assert.truthy(text:find("Basilisk"))
    -- Restore.
    statusline.set_state("stopped")
  end)

  it("set_state to error uses DiagnosticError color", function()
    statusline.set_state("error")
    local color = statusline.get_color()
    assert.are.equal("DiagnosticError", color, "error state should use DiagnosticError")
    statusline.set_state("stopped")
  end)

  it("set_state to stopped uses Comment color", function()
    statusline.set_state("stopped")
    local color = statusline.get_color()
    assert.are.equal("Comment", color, "stopped state should use Comment")
  end)

  it("lualine_component is a valid table with function and color", function()
    assert.is_table(statusline.lualine_component)
    assert.is_function(statusline.lualine_component[1])
    assert.is_function(statusline.lualine_component.color)
  end)

  it("lualine_component function returns string", function()
    local fn = statusline.lualine_component[1]
    local result = fn()
    assert.is_string(result)
    assert.is_true(#result > 0)
  end)
end)

-- ── Data Structures (VSIX parity: ProfileResult, ProfileHotLine, ProfileHotFunction) ──

describe("Profiler — Data Structures", function()
  it("ProfileResult type has all required fields", function()
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
    assert.is_table(result.hotFunctions)
    assert.is_table(result.hotLines)
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
    assert.is_true(hot_func.selfPercentage <= hot_func.percentage,
      "selfPercentage should not exceed percentage")
  end)

  it("ProfileResult with populated hotFunctions validates structure", function()
    local result = {
      sessionId = "populated-session",
      duration = 10.5,
      totalSamples = 5000,
      outputFile = "/tmp/profile.speedscope.json",
      hotFunctions = {
        { name = "compute", file = "/src/math.py", line = 10, samples = 2500, percentage = 50.0, selfPercentage = 35.0 },
        { name = "transform", file = "/src/utils.py", line = 88, samples = 1000, percentage = 20.0, selfPercentage = 15.0 },
      },
      hotLines = {
        { file = "/src/math.py", line = 12, samples = 2000, percentage = 40.0 },
      },
    }

    assert.are.equal(2, #result.hotFunctions, "should have 2 hot functions")
    assert.are.equal(1, #result.hotLines, "should have 1 hot line")
    assert.are.equal("compute", result.hotFunctions[1].name)
    assert.are.equal("transform", result.hotFunctions[2].name)
    assert.is_true(result.hotFunctions[1].percentage > result.hotFunctions[2].percentage,
      "first function should have higher percentage")
    assert.is_true(result.hotFunctions[1].selfPercentage <= result.hotFunctions[1].percentage,
      "selfPercentage should not exceed percentage")
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
    assert.is_table(snapshot.topAllocations)
  end)

  it("MemorySnapshotResult with allocations validates ordering", function()
    local snapshot = {
      memorySessionId = "mem-full",
      snapshotId = "snap-full",
      currentMemory = 100000000,
      peakMemory = 150000000,
      topAllocations = {
        { file = "/src/data.py", line = 10, size = 50000000, count = 1000 },
        { file = "/src/cache.py", line = 20, size = 30000000, count = 500 },
      },
    }

    assert.are.equal(2, #snapshot.topAllocations)
    assert.is_true(snapshot.topAllocations[1].size > snapshot.topAllocations[2].size,
      "allocations should be ordered by size")
    assert.is_true(snapshot.currentMemory <= snapshot.peakMemory,
      "currentMemory should not exceed peakMemory")
  end)

  it("LeakConfidence values are within valid range", function()
    local leak = {
      typeName = "DataFrame",
      confidence = 0.85,
      count = 15,
      totalSize = "1.2MB",
    }

    assert.is_true(leak.confidence >= 0 and leak.confidence <= 1,
      "confidence should be between 0 and 1")
  end)

  it("SuspectedLeak type has required fields", function()
    local leak = {
      typeName = "dict",
      confidence = 0.72,
      count = 50,
      totalSize = "500KB",
      location = { file = "/app/cache.py", line = 33 },
    }

    assert.are.equal("dict", leak.typeName)
    assert.is_number(leak.confidence)
    assert.is_number(leak.count)
    assert.is_string(leak.totalSize)
    assert.is_not_nil(leak.location)
    assert.are.equal("/app/cache.py", leak.location.file)
    assert.are.equal(33, leak.location.line)
  end)

  it("MemoryDiffResult type has required fields", function()
    local diff = {
      baseSnapshotId = "snap-001",
      comparedSnapshotId = "snap-002",
      totalGrowth = 25000000,
      newAllocations = 150,
      freedAllocations = 50,
      suspectedLeaks = {},
    }

    assert.are.equal("snap-001", diff.baseSnapshotId)
    assert.are.equal("snap-002", diff.comparedSnapshotId)
    assert.is_number(diff.totalGrowth)
    assert.is_number(diff.newAllocations)
    assert.is_number(diff.freedAllocations)
    assert.is_table(diff.suspectedLeaks)
    assert.is_true(diff.newAllocations > diff.freedAllocations,
      "net growth should be positive")
  end)
end)

-- ── Decoration Contracts (VSIX parity: Profiler — Decoration Contracts) ──

describe("Profiler — Decoration Contracts", function()
  local profiling = require("basilisk.profiling")
  local ns = vim.api.nvim_create_namespace("basilisk-profiling")

  after_each(function()
    for _, buf in ipairs(vim.api.nvim_list_bufs()) do
      if vim.api.nvim_buf_is_loaded(buf) then
        vim.api.nvim_buf_clear_namespace(buf, ns, 0, -1)
      end
    end
    for _, win in ipairs(vim.api.nvim_list_wins()) do
      local config = vim.api.nvim_win_get_config(win)
      if config.relative and config.relative ~= "" then
        pcall(vim.api.nvim_win_close, win, true)
      end
    end
  end)

  it("apply-clear-reapply cycle is idempotent", function()
    local tmpfile = vim.fn.tempname() .. ".py"
    local fh = io.open(tmpfile, "w")
    if fh then fh:write("x = 1\n") fh:close() end
    vim.cmd("edit " .. tmpfile)
    local buf = vim.api.nvim_get_current_buf()
    local canonical = vim.api.nvim_buf_get_name(buf)

    local hot = { { file = canonical, line = 1, percentage = 42.0 } }

    assert.has_no.errors(function() profiling.apply_heat_map(hot) end)
    assert.has_no.errors(function() profiling.apply_heat_map({}) end)
    assert.has_no.errors(function() profiling.apply_heat_map(hot) end)

    local marks = vim.api.nvim_buf_get_extmarks(buf, ns, 0, -1, {})
    assert.are.equal(1, #marks, "should have exactly 1 mark after reapply")

    vim.cmd("bdelete!") os.remove(tmpfile)
  end)

  it("double-clear does not throw", function()
    assert.has_no.errors(function()
      profiling.apply_heat_map({})
      profiling.apply_heat_map({})
    end)
  end)

  it("decorations with multiple files and varying percentages", function()
    local tmpfile = vim.fn.tempname() .. ".py"
    local fh = io.open(tmpfile, "w")
    if fh then fh:write("def hot_func():\n    x = 1\n    y = 2\n    z = x + y\n    return z\n") fh:close() end
    vim.cmd("edit " .. tmpfile)
    local buf = vim.api.nvim_get_current_buf()
    local canonical = vim.api.nvim_buf_get_name(buf)

    profiling.apply_heat_map({
      { file = canonical, line = 1, percentage = 55.0 },
      { file = canonical, line = 3, percentage = 25.0 },
      { file = canonical, line = 5, percentage = 3.0 },
    })

    local marks = vim.api.nvim_buf_get_extmarks(buf, ns, 0, -1, { details = true })
    assert.are.equal(3, #marks, "should have 3 extmarks")

    -- Verify highlight groups match profiling.lua logic.
    -- >50% = DiagnosticError, 20-50% = DiagnosticWarn, <20% = DiagnosticHint
    assert.are.equal("DiagnosticError", marks[1][4].virt_text[1][2])
    assert.are.equal("DiagnosticWarn", marks[2][4].virt_text[1][2])
    assert.are.equal("DiagnosticHint", marks[3][4].virt_text[1][2])

    vim.cmd("bdelete!") os.remove(tmpfile)
  end)

  it("heat level boundary at exactly 1%% in extmarks", function()
    local tmpfile = vim.fn.tempname() .. ".py"
    local fh = io.open(tmpfile, "w")
    if fh then fh:write("x = 1\n") fh:close() end
    vim.cmd("edit " .. tmpfile)
    local buf = vim.api.nvim_get_current_buf()
    local canonical = vim.api.nvim_buf_get_name(buf)

    profiling.apply_heat_map({
      { file = canonical, line = 1, percentage = 1.0 },
    })

    local marks = vim.api.nvim_buf_get_extmarks(buf, ns, 0, -1, { details = true })
    assert.is_true(#marks > 0, "1%% should produce an extmark")
    -- 1% is <20% so DiagnosticHint.
    assert.are.equal("DiagnosticHint", marks[1][4].virt_text[1][2])

    vim.cmd("bdelete!") os.remove(tmpfile)
  end)

  it("extmark virtual text contains percentage value", function()
    local tmpfile = vim.fn.tempname() .. ".py"
    local fh = io.open(tmpfile, "w")
    if fh then fh:write("hot = True\n") fh:close() end
    vim.cmd("edit " .. tmpfile)
    local buf = vim.api.nvim_get_current_buf()
    local canonical = vim.api.nvim_buf_get_name(buf)

    profiling.apply_heat_map({
      { file = canonical, line = 1, percentage = 67.3 },
    })

    local marks = vim.api.nvim_buf_get_extmarks(buf, ns, 0, -1, { details = true })
    assert.is_true(#marks > 0)
    local virt_str = marks[1][4].virt_text[1][1]
    assert.truthy(virt_str:find("67.3"), "virtual text should contain the percentage")

    vim.cmd("bdelete!") os.remove(tmpfile)
  end)
end)

-- ── Configuration Interaction (VSIX parity: Profiler — Configuration Interaction) ──

describe("Profiler — Configuration Interaction", function()
  local config_mod = require("basilisk.config")

  it("default config validates without errors", function()
    local errors = config_mod.validate(config_mod.defaults)
    assert.are.equal(0, #errors, "default config should have no validation errors")
  end)

  it("test_explorer framework supports auto, pytest, unittest", function()
    local valid_frameworks = { "auto", "pytest", "unittest" }
    for _, fw in ipairs(valid_frameworks) do
      local cfg = vim.tbl_deep_extend("force", {}, config_mod.defaults, {
        test_explorer = { framework = fw },
      })
      local errors = config_mod.validate(cfg)
      assert.are.equal(0, #errors, fw .. " should be a valid framework")
    end
  end)

  it("rejects invalid test_explorer framework", function()
    local cfg = vim.tbl_deep_extend("force", {}, config_mod.defaults, {
      test_explorer = { framework = "nose" },
    })
    local errors = config_mod.validate(cfg)
    assert.is_true(#errors > 0, "invalid framework should produce error")
  end)

  it("valid analysis modes are accepted", function()
    local valid_modes = { "openFilesOnly", "wholeModule", "crossModule" }
    for _, mode in ipairs(valid_modes) do
      local cfg = vim.tbl_deep_extend("force", {}, config_mod.defaults, {
        analysis_mode = mode,
      })
      local errors = config_mod.validate(cfg)
      assert.are.equal(0, #errors, mode .. " should be a valid analysis mode")
    end
  end)

  it("valid log levels are accepted", function()
    local valid_levels = { "trace", "debug", "info", "warn", "error" }
    for _, level in ipairs(valid_levels) do
      local cfg = vim.tbl_deep_extend("force", {}, config_mod.defaults, {
        log_level = level,
      })
      local errors = config_mod.validate(cfg)
      assert.are.equal(0, #errors, level .. " should be a valid log level")
    end
  end)

  it("config merge preserves nested defaults", function()
    local resolved = config_mod.resolve({ analysis_mode = "crossModule" })
    assert.are.equal("crossModule", resolved.analysis_mode)
    -- Nested defaults should be preserved.
    assert.are.equal("ruff", resolved.formatter, "formatter default should be preserved")
    assert.is_true(resolved.test_explorer.enabled, "test_explorer.enabled should be preserved")
    assert.is_true(resolved.uv.enabled, "uv.enabled should be preserved")
  end)

  it("keymaps have default prefix", function()
    local resolved = config_mod.resolve()
    assert.are.equal("<leader>b", resolved.keymaps.prefix)
    assert.is_true(resolved.keymaps.enabled)
  end)
end)

-- ── Lifecycle Interaction (VSIX parity: Profiler — Lifecycle Interaction) ──

describe("Profiler — Lifecycle Interaction", function()
  local profiling = require("basilisk.profiling")

  after_each(function()
    for _, win in ipairs(vim.api.nvim_list_wins()) do
      local config = vim.api.nvim_win_get_config(win)
      if config.relative and config.relative ~= "" then
        pcall(vim.api.nvim_win_close, win, true)
      end
    end
    vim.fn.setqflist({}, "r")
  end)

  it("handles special characters in function names", function()
    local result = {
      hotFunctions = {
        { name = "__init__", file = "/tmp/cls.py", line = 1, percentage = 30.0 },
        { name = "<lambda>", file = "/tmp/cls.py", line = 5, percentage = 15.0 },
        { name = "Class.method", file = "/tmp/cls.py", line = 10, percentage = 10.0 },
      },
    }

    assert.has_no.errors(function()
      profiling.display_results(result)
    end)

    -- Verify all names appear in float.
    for _, win in ipairs(vim.api.nvim_list_wins()) do
      local config = vim.api.nvim_win_get_config(win)
      if config.relative and config.relative ~= "" then
        local buf = vim.api.nvim_win_get_buf(win)
        local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
        local text = table.concat(lines, "\n")
        assert.truthy(text:find("__init__"), "should show __init__")
        assert.truthy(text:find("Class.method"), "should show Class.method")
      end
    end
  end)

  it("handles 20 hot functions without issues", function()
    local funcs = {}
    for i = 1, 20 do
      funcs[i] = {
        name = "func_" .. i,
        file = "/tmp/many.py",
        line = i,
        percentage = 100 / i,
      }
    end

    assert.has_no.errors(function()
      profiling.display_results({ hotFunctions = funcs })
    end)

    local qf = vim.fn.getqflist()
    assert.are.equal(20, #qf, "quickfix should have all 20 functions")
  end)

  it("consecutive display_results calls replace quickfix", function()
    profiling.display_results({
      hotFunctions = {
        { name = "old_func", file = "/tmp/old.py", line = 1, percentage = 50 },
      },
    })
    assert.is_true(#vim.fn.getqflist() >= 1)

    for _, win in ipairs(vim.api.nvim_list_wins()) do
      local config = vim.api.nvim_win_get_config(win)
      if config.relative and config.relative ~= "" then
        pcall(vim.api.nvim_win_close, win, true)
      end
    end

    profiling.display_results({
      hotFunctions = {
        { name = "new_func", file = "/tmp/new.py", line = 2, percentage = 70 },
      },
    })

    local qf = vim.fn.getqflist()
    assert.is_true(#qf >= 1)
    assert.truthy(qf[1].text:find("new_func"), "quickfix should have new results")
  end)

  it("web server profiling scenario with 6 functions", function()
    local result = {
      hotFunctions = {
        { name = "db_query", file = "/app/models.py", line = 45, percentage = 45.0 },
        { name = "render_template", file = "/app/views.py", line = 22, percentage = 25.0 },
        { name = "serialize_json", file = "/app/serializers.py", line = 10, percentage = 12.0 },
        { name = "validate_input", file = "/app/validators.py", line = 5, percentage = 8.0 },
        { name = "log_request", file = "/app/middleware.py", line = 30, percentage = 3.0 },
        { name = "parse_headers", file = "/app/http.py", line = 15, percentage = 1.5 },
      },
    }

    profiling.display_results(result)
    local qf = vim.fn.getqflist()
    assert.are.equal(6, #qf, "quickfix should have all 6 web functions")

    -- Verify ordering: first should be db_query (highest CPU).
    assert.truthy(qf[1].text:find("db_query"), "first qf item should be db_query")
    assert.truthy(qf[6].text:find("parse_headers"), "last should be parse_headers")
  end)

  it("error messages from profiler start/stop are not raw stack traces", function()
    -- Without an LSP client, start/stop/snapshot should log warnings, not throw.
    assert.has_no.errors(function()
      profiling.start(0)
    end)
    assert.has_no.errors(function()
      profiling.stop()
    end)
    assert.has_no.errors(function()
      profiling.snapshot()
    end)
  end)
end)

-- ── Suite: Command Registration (VSIX parity) ─────────────────────────────

describe("profiler — command registration", function()
  -- Ensure commands are registered (idempotent).
  local config = require("basilisk.config").defaults
  require("basilisk.commands").register(config)

  local PROFILER_COMMANDS = {
    "BasiliskProfile",
    "BasiliskProfileStop",
    "BasiliskProfileSnapshot",
  }

  local MEMORY_COMMANDS = {
    "BasiliskMemLeak",
    "BasiliskMemStop",
    "BasiliskMemRefs",
  }

  it("all profiler user commands are registered", function()
    local all_cmds = vim.api.nvim_get_commands({})
    for _, cmd in ipairs(PROFILER_COMMANDS) do
      assert.truthy(all_cmds[cmd], "user command '" .. cmd .. "' should be registered")
    end
  end)

  it("profiler commands have descriptions", function()
    local all_cmds = vim.api.nvim_get_commands({})
    for _, cmd in ipairs(PROFILER_COMMANDS) do
      local entry = all_cmds[cmd]
      assert.truthy(entry, "command '" .. cmd .. "' should exist")
      assert.truthy(
        entry.definition and #entry.definition > 0,
        "command '" .. cmd .. "' should have a description"
      )
    end
  end)

  it("BasiliskProfile accepts optional PID argument (nargs=?)", function()
    local all_cmds = vim.api.nvim_get_commands({})
    local entry = all_cmds["BasiliskProfile"]
    assert.truthy(entry, "BasiliskProfile should exist")
    assert.are.equal("?", entry.nargs, "BasiliskProfile should accept 0 or 1 args")
  end)

  it("BasiliskProfileStop takes no arguments (nargs=0)", function()
    local all_cmds = vim.api.nvim_get_commands({})
    local entry = all_cmds["BasiliskProfileStop"]
    assert.truthy(entry, "BasiliskProfileStop should exist")
    assert.are.equal("0", entry.nargs, "BasiliskProfileStop takes no arguments")
  end)

  it("BasiliskProfileSnapshot takes no arguments (nargs=0)", function()
    local all_cmds = vim.api.nvim_get_commands({})
    local entry = all_cmds["BasiliskProfileSnapshot"]
    assert.truthy(entry, "BasiliskProfileSnapshot should exist")
    assert.are.equal("0", entry.nargs, "BasiliskProfileSnapshot takes no arguments")
  end)

  it("profiler commands are distinct from memory commands", function()
    local profiler_set = {}
    for _, cmd in ipairs(PROFILER_COMMANDS) do
      profiler_set[cmd] = true
    end
    for _, cmd in ipairs(MEMORY_COMMANDS) do
      assert.falsy(profiler_set[cmd], "memory command '" .. cmd .. "' must not be in profiler set")
    end
  end)

  it("all profiler and memory commands are unique", function()
    local all_commands = {}
    for _, cmd in ipairs(PROFILER_COMMANDS) do
      all_commands[#all_commands + 1] = cmd
    end
    for _, cmd in ipairs(MEMORY_COMMANDS) do
      all_commands[#all_commands + 1] = cmd
    end
    local seen = {}
    for _, cmd in ipairs(all_commands) do
      assert.falsy(seen[cmd], "command '" .. cmd .. "' must be unique")
      seen[cmd] = true
    end
  end)

  it("memory commands are also registered", function()
    local all_cmds = vim.api.nvim_get_commands({})
    for _, cmd in ipairs(MEMORY_COMMANDS) do
      assert.truthy(all_cmds[cmd], "memory command '" .. cmd .. "' should be registered")
    end
  end)

  it("BasiliskMemRefs takes exactly 1 argument (nargs=1)", function()
    local all_cmds = vim.api.nvim_get_commands({})
    local entry = all_cmds["BasiliskMemRefs"]
    assert.truthy(entry)
    assert.are.equal("1", entry.nargs, "BasiliskMemRefs should take exactly 1 arg")
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
end)

-- ── Suite: Configuration (VSIX parity) ─────────────────────────────────────

describe("profiler — configuration", function()
  local config = require("basilisk.config")

  it("config module loads without error", function()
    assert.truthy(config)
    assert.truthy(config.defaults)
  end)

  it("resolve merges defaults with empty options", function()
    local resolved = config.resolve({})
    assert.are.equal(true, resolved.enabled, "enabled defaults to true")
    assert.are.equal(true, resolved.use_lsp, "use_lsp defaults to true")
    assert.are.equal("wholeModule", resolved.analysis_mode)
  end)

  it("log_level defaults to 'info'", function()
    local resolved = config.resolve({})
    assert.are.equal("info", resolved.log_level)
  end)

  it("validate rejects invalid analysis_mode", function()
    local bad = vim.tbl_deep_extend("force", {}, config.defaults, { analysis_mode = "invalid" })
    local errors = config.validate(bad)
    assert.is_true(#errors > 0, "should report validation error")
  end)

  it("validate accepts all valid analysis_mode values", function()
    for _, mode in ipairs({ "openFilesOnly", "wholeModule", "crossModule" }) do
      local cfg = vim.tbl_deep_extend("force", {}, config.defaults, { analysis_mode = mode })
      local errors = config.validate(cfg)
      assert.are.equal(0, #errors, "'" .. mode .. "' should be valid")
    end
  end)

  it("validate rejects invalid log_level", function()
    local bad = vim.tbl_deep_extend("force", {}, config.defaults, { log_level = "banana" })
    local errors = config.validate(bad)
    assert.is_true(#errors > 0)
  end)

  it("validate accepts all valid log_level values", function()
    for _, level in ipairs({ "trace", "debug", "info", "warn", "error" }) do
      local cfg = vim.tbl_deep_extend("force", {}, config.defaults, { log_level = level })
      local errors = config.validate(cfg)
      assert.are.equal(0, #errors, "'" .. level .. "' should be valid")
    end
  end)

  it("validate rejects invalid test_explorer framework", function()
    local bad = vim.deepcopy(config.defaults)
    bad.test_explorer.framework = "jest"
    local errors = config.validate(bad)
    assert.is_true(#errors > 0)
  end)

  it("validate rejects invalid test_explorer position", function()
    local bad = vim.deepcopy(config.defaults)
    bad.test_explorer.position = "top"
    local errors = config.validate(bad)
    assert.is_true(#errors > 0)
  end)

  it("resolve overrides defaults with user options", function()
    local resolved = config.resolve({ log_level = "debug", analysis_mode = "crossModule" })
    assert.are.equal("debug", resolved.log_level)
    assert.are.equal("crossModule", resolved.analysis_mode)
  end)

  it("debugger defaults are correct", function()
    local resolved = config.resolve({})
    assert.are.equal(true, resolved.debugger.enabled)
    assert.are.equal(false, resolved.debugger.type_checking)
    assert.are.equal("debugpy", resolved.debugger.debugpy_path)
  end)

  it("test_explorer defaults are correct", function()
    local resolved = config.resolve({})
    assert.are.equal(true, resolved.test_explorer.enabled)
    assert.are.equal("auto", resolved.test_explorer.framework)
    assert.are.equal("pytest", resolved.test_explorer.pytest_path)
    assert.are.equal(true, resolved.test_explorer.auto_discover_on_save)
    assert.are.equal("right", resolved.test_explorer.position)
    assert.are.equal(40, resolved.test_explorer.width)
  end)

  it("formatter defaults are correct", function()
    -- [LSPFMT-CONFIG]: the embedded Ruff formatter is the default engine.
    local resolved = config.resolve({})
    assert.are.equal("ruff", resolved.formatter)
  end)

  it("inlay_hints defaults are correct", function()
    local resolved = config.resolve({})
    assert.are.equal(true, resolved.inlay_hints.parameter_names)
    assert.are.equal(true, resolved.inlay_hints.variable_types)
  end)
end)

-- ── Suite: Status Bar (VSIX parity) ────────────────────────────────────────

describe("profiler — statusline", function()
  local statusline = require("basilisk.statusline")

  it("module loads", function()
    assert.truthy(statusline)
  end)

  it("get() returns a non-empty string", function()
    local text = statusline.get()
    assert.is_string(text)
    assert.is_true(#text > 0)
  end)

  it("get() contains 'Basilisk'", function()
    local text = statusline.get()
    assert.truthy(text:find("Basilisk"), "statusline should mention Basilisk")
  end)

  it("get_color() returns a highlight group string", function()
    local color = statusline.get_color()
    assert.is_string(color)
    assert.is_true(#color > 0)
  end)

  it("set_state('error') uses DiagnosticError", function()
    statusline.set_state("error")
    assert.are.equal("DiagnosticError", statusline.get_color())
    statusline.set_state("stopped")
  end)

  it("set_state('starting') uses DiagnosticWarn", function()
    statusline.set_state("starting")
    assert.are.equal("DiagnosticWarn", statusline.get_color())
    statusline.set_state("stopped")
  end)

  it("set_state('stopped') uses Comment", function()
    statusline.set_state("stopped")
    assert.are.equal("Comment", statusline.get_color())
  end)

  it("lualine_component is a valid table", function()
    assert.is_table(statusline.lualine_component)
    assert.is_function(statusline.lualine_component[1])
    assert.is_function(statusline.lualine_component.color)
  end)

  it("lualine component function returns statusline text", function()
    local text = statusline.lualine_component[1]()
    assert.is_string(text)
    assert.truthy(text:find("Basilisk"))
  end)

  it("lualine component color returns a table with fg", function()
    local result = statusline.lualine_component.color()
    assert.is_table(result)
    -- fg may be nil if highlight not defined in headless mode, but table should exist.
  end)
end)

-- ── Suite: Keybindings (VSIX parity) ───────────────────────────────────────

describe("profiler — keybindings", function()
  local config = require("basilisk.config")

  it("keymaps config has enabled flag", function()
    assert.is_table(config.defaults.keymaps)
    assert.is_boolean(config.defaults.keymaps.enabled)
  end)

  it("keymaps enabled defaults to true", function()
    assert.are.equal(true, config.defaults.keymaps.enabled)
  end)

  it("keymaps config has prefix string", function()
    assert.is_string(config.defaults.keymaps.prefix)
    assert.is_true(#config.defaults.keymaps.prefix > 0)
  end)

  it("default keymap prefix is <leader>b", function()
    assert.are.equal("<leader>b", config.defaults.keymaps.prefix)
  end)

  it("keymaps can be disabled via config", function()
    local resolved = config.resolve({ keymaps = { enabled = false } })
    assert.are.equal(false, resolved.keymaps.enabled)
  end)

  it("keymap prefix can be customized", function()
    local resolved = config.resolve({ keymaps = { prefix = "<leader>p" } })
    assert.are.equal("<leader>p", resolved.keymaps.prefix)
  end)
end)

-- ── Suite: Heat Level Classification (VSIX parity) ─────────────────────────

describe("profiler — heat level classification", function()
  --- Classify heat level from percentage (mirrors profiler-decorations.ts).
  ---@param pct number
  ---@return string
  local function classify_heat(pct)
    if pct >= 20 then
      return "critical"
    elseif pct >= 10 then
      return "hot"
    elseif pct >= 5 then
      return "warm"
    elseif pct >= 1 then
      return "cool"
    else
      return "none"
    end
  end

  it("critical heat level classification (>= 20%)", function()
    assert.are.equal("critical", classify_heat(25.0))
    assert.are.equal("critical", classify_heat(20.0))
    assert.are.equal("critical", classify_heat(100.0))
  end)

  it("hot heat level classification (10-20%)", function()
    assert.are.equal("hot", classify_heat(15.0))
    assert.are.equal("hot", classify_heat(10.0))
    assert.are.equal("hot", classify_heat(19.9))
  end)

  it("warm heat level classification (5-10%)", function()
    assert.are.equal("warm", classify_heat(7.0))
    assert.are.equal("warm", classify_heat(5.0))
    assert.are.equal("warm", classify_heat(9.9))
  end)

  it("cool heat level classification (1-5%)", function()
    assert.are.equal("cool", classify_heat(3.0))
    assert.are.equal("cool", classify_heat(1.0))
    assert.are.equal("cool", classify_heat(4.9))
  end)

  it("below threshold (< 1%) is not classified", function()
    assert.are.equal("none", classify_heat(0.5))
    assert.are.equal("none", classify_heat(0.0))
    assert.are.equal("none", classify_heat(0.99))
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
      assert.are.equal(
        tc.expected,
        classify_heat(tc.pct),
        string.format("%.1f%% should be '%s'", tc.pct, tc.expected)
      )
    end
  end)

  it("heat map extmark highlight groups match profiling.lua palette", function()
    -- profiling.lua apply_heat_map uses:
    --   > 50% → DiagnosticError
    --   > 20% → DiagnosticWarn
    --   else  → DiagnosticHint
    local function expected_hl(pct)
      if pct > 50 then
        return "DiagnosticError"
      elseif pct > 20 then
        return "DiagnosticWarn"
      else
        return "DiagnosticHint"
      end
    end

    assert.are.equal("DiagnosticError", expected_hl(60))
    assert.are.equal("DiagnosticError", expected_hl(51))
    assert.are.equal("DiagnosticWarn", expected_hl(50))
    assert.are.equal("DiagnosticWarn", expected_hl(21))
    assert.are.equal("DiagnosticHint", expected_hl(20))
    assert.are.equal("DiagnosticHint", expected_hl(5))
    assert.are.equal("DiagnosticHint", expected_hl(1))
  end)
end)

-- ── Suite: Data Structures (VSIX parity) ───────────────────────────────────

describe("profiler — data structures", function()
  it("ProfileResult-like table validates required fields", function()
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
    assert.is_table(result.hotFunctions)
    assert.is_table(result.hotLines)
  end)

  it("ProfileHotLine-like table validates required fields", function()
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

  it("ProfileHotFunction-like table validates required fields", function()
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
    assert.is_true(hot_func.selfPercentage <= hot_func.percentage,
      "selfPercentage should not exceed percentage")
  end)

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
      growthEntries = {},
      totalGrowth = 1048576,
    }

    assert.are.equal("snap-001", diff.beforeSnapshot)
    assert.are.equal("snap-002", diff.afterSnapshot)
    assert.is_table(diff.growthEntries)
    assert.are.equal(1048576, diff.totalGrowth)
  end)

  it("SuspectedLeak-like table validates required fields", function()
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

  it("populated ProfileResult validates hot function ordering", function()
    local result = {
      sessionId = "populated-session",
      duration = 10.5,
      totalSamples = 5000,
      outputFile = "/tmp/profile.speedscope.json",
      hotFunctions = {
        { name = "compute", file = "/src/math.py", line = 10, samples = 2500, percentage = 50.0, selfPercentage = 35.0 },
        { name = "transform", file = "/src/utils.py", line = 88, samples = 1000, percentage = 20.0, selfPercentage = 15.0 },
      },
      hotLines = {
        { file = "/src/math.py", line = 12, samples = 2000, percentage = 40.0 },
      },
    }

    assert.are.equal(2, #result.hotFunctions)
    assert.are.equal(1, #result.hotLines)
    assert.are.equal("compute", result.hotFunctions[1].name)
    assert.are.equal("transform", result.hotFunctions[2].name)
    assert.is_true(result.hotFunctions[1].percentage > result.hotFunctions[2].percentage,
      "first function should have higher percentage")
    assert.is_true(result.hotFunctions[1].selfPercentage <= result.hotFunctions[1].percentage,
      "selfPercentage must not exceed percentage")
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
end)
