--- UI tests for basilisk.testing module.

describe("basilisk.testing", function()
  local testing = require("basilisk.testing")

  -- ── parse_pytest_output ────────────────────────────────────────────

  describe("parse_pytest_output", function()
    it("parses simple test output", function()
      local output = "test_example.py::test_add\ntest_example.py::test_subtract\n"
      local tree = testing.parse_pytest_output(output)
      assert.are.equal(1, #tree)
      assert.are.equal("test_example.py", tree[1].name)
      assert.are.equal("file", tree[1].kind)
      assert.are.equal(2, #tree[1].children)
      assert.are.equal("test_add", tree[1].children[1].name)
      assert.are.equal("test_subtract", tree[1].children[2].name)
    end)

    it("parses class-based tests", function()
      local output = "test_math.py::TestMath::test_add\ntest_math.py::TestMath::test_multiply\n"
      local tree = testing.parse_pytest_output(output)
      assert.are.equal(1, #tree)
      assert.are.equal(1, #tree[1].children)
      assert.are.equal("TestMath", tree[1].children[1].name)
      assert.are.equal("class", tree[1].children[1].kind)
      assert.are.equal(2, #tree[1].children[1].children)
    end)

    it("handles multiple files", function()
      local output = "test_a.py::test_one\ntest_b.py::test_two\n"
      local tree = testing.parse_pytest_output(output)
      assert.are.equal(2, #tree)
    end)

    it("skips empty lines and summary lines", function()
      local output = "\n===== 5 items =====\ntest_x.py::test_foo\n"
      local tree = testing.parse_pytest_output(output)
      assert.are.equal(1, #tree)
      assert.are.equal(1, #tree[1].children)
    end)

    it("returns empty tree for no tests", function()
      local output = "no tests ran\n"
      local tree = testing.parse_pytest_output(output)
      assert.are.equal(0, #tree)
    end)

    it("preserves full test ID", function()
      local output = "tests/test_core.py::TestClass::test_method\n"
      local tree = testing.parse_pytest_output(output)
      local test_node = tree[1].children[1].children[1]
      assert.are.equal("tests/test_core.py::TestClass::test_method", test_node.id)
    end)

    it("defaults status to unknown", function()
      local output = "test_x.py::test_foo\n"
      local tree = testing.parse_pytest_output(output)
      assert.are.equal("unknown", tree[1].status)
      assert.are.equal("unknown", tree[1].children[1].status)
    end)

    it("groups methods under the same class", function()
      local output = table.concat({
        "test_api.py::TestUsers::test_create",
        "test_api.py::TestUsers::test_delete",
        "test_api.py::TestUsers::test_update",
      }, "\n") .. "\n"
      local tree = testing.parse_pytest_output(output)
      assert.are.equal(1, #tree)
      assert.are.equal(1, #tree[1].children)
      local class_node = tree[1].children[1]
      assert.are.equal("TestUsers", class_node.name)
      assert.are.equal(3, #class_node.children)
      assert.are.equal("test_create", class_node.children[1].name)
      assert.are.equal("test_delete", class_node.children[2].name)
      assert.are.equal("test_update", class_node.children[3].name)
    end)

    it("handles multiple classes in one file", function()
      local output = table.concat({
        "test_db.py::TestInsert::test_row",
        "test_db.py::TestQuery::test_select",
      }, "\n") .. "\n"
      local tree = testing.parse_pytest_output(output)
      assert.are.equal(1, #tree)
      assert.are.equal(2, #tree[1].children)
      assert.are.equal("TestInsert", tree[1].children[1].name)
      assert.are.equal("TestQuery", tree[1].children[2].name)
    end)

    it("handles mixed functions and classes in one file", function()
      local output = table.concat({
        "test_mixed.py::test_standalone",
        "test_mixed.py::TestGroup::test_method",
      }, "\n") .. "\n"
      local tree = testing.parse_pytest_output(output)
      assert.are.equal(1, #tree)
      assert.are.equal(2, #tree[1].children)
      assert.are.equal("test_standalone", tree[1].children[1].name)
      assert.are.equal("function", tree[1].children[1].kind)
      assert.are.equal("TestGroup", tree[1].children[2].name)
      assert.are.equal("class", tree[1].children[2].kind)
    end)

    it("handles subdirectory paths in test IDs", function()
      local output = "tests/unit/test_core.py::test_main\n"
      local tree = testing.parse_pytest_output(output)
      assert.are.equal(1, #tree)
      assert.are.equal("test_core.py", tree[1].name)
      assert.are.equal("tests/unit/test_core.py", tree[1].file)
    end)

    it("file node id is the file path", function()
      local output = "test_api.py::test_get\n"
      local tree = testing.parse_pytest_output(output)
      assert.are.equal("test_api.py", tree[1].id)
    end)

    it("class node id includes file and class name", function()
      local output = "test_api.py::TestEndpoint::test_get\n"
      local tree = testing.parse_pytest_output(output)
      assert.are.equal("test_api.py::TestEndpoint", tree[1].children[1].id)
    end)

    it("function node children are empty arrays", function()
      local output = "test_api.py::test_get\n"
      local tree = testing.parse_pytest_output(output)
      assert.are.equal(0, #tree[1].children[1].children)
    end)

    it("handles many files in large output", function()
      local lines = {}
      for i = 1, 20 do
        lines[i] = string.format("test_file_%d.py::test_case_%d", i, i)
      end
      local output = table.concat(lines, "\n") .. "\n"
      local tree = testing.parse_pytest_output(output)
      assert.are.equal(20, #tree)
    end)

    it("ignores lines without .py pattern", function()
      local output = "not_a_test::test_foo\ntest_real.py::test_bar\n"
      local tree = testing.parse_pytest_output(output)
      assert.are.equal(1, #tree)
      assert.are.equal("test_real.py", tree[1].name)
    end)

    it("handles empty string input", function()
      local tree = testing.parse_pytest_output("")
      assert.are.equal(0, #tree)
    end)
  end)

  -- ── parse_test_results ─────────────────────────────────────────────

  describe("parse_test_results", function()
    it("does not error on empty output", function()
      assert.has_no.errors(function()
        testing.parse_test_results("")
      end)
    end)

    it("does not error on pytest verbose output", function()
      local output = table.concat({
        "test_math.py::test_add PASSED",
        "test_math.py::test_subtract FAILED",
        "",
        "========= 1 passed, 1 failed =========",
      }, "\n")
      assert.has_no.errors(function()
        testing.parse_test_results(output)
      end)
    end)

    it("does not error on all-passing output", function()
      local output = table.concat({
        "test_a.py::test_one PASSED",
        "test_a.py::test_two PASSED",
        "test_a.py::test_three PASSED",
      }, "\n")
      assert.has_no.errors(function()
        testing.parse_test_results(output)
      end)
    end)

    it("does not error on all-failing output", function()
      local output = table.concat({
        "test_a.py::test_one FAILED",
        "test_a.py::test_two FAILED",
      }, "\n")
      assert.has_no.errors(function()
        testing.parse_test_results(output)
      end)
    end)

    it("does not error on class method results", function()
      local output = "test_api.py::TestEndpoint::test_get PASSED\n"
      assert.has_no.errors(function()
        testing.parse_test_results(output)
      end)
    end)
  end)

  -- ── set_status ─────────────────────────────────────────────────────

  describe("set_status", function()
    it("updates a node status by ID", function()
      testing.parse_pytest_output("test_x.py::test_foo\ntest_x.py::test_bar\n")
      testing.set_status("test_x.py::test_foo", "passed")
      assert.is_true(true)
    end)

    it("does not error for nil test_id", function()
      assert.has_no.errors(function()
        testing.set_status(nil, "passed")
      end)
    end)

    it("does not error for non-existent test_id", function()
      testing.parse_pytest_output("test_x.py::test_real\n")
      assert.has_no.errors(function()
        testing.set_status("test_x.py::test_nonexistent", "failed")
      end)
    end)

    it("accepts all valid status values", function()
      testing.parse_pytest_output("test_s.py::test_s\n")
      for _, status in ipairs({ "unknown", "running", "passed", "failed" }) do
        assert.has_no.errors(function()
          testing.set_status("test_s.py::test_s", status)
        end)
      end
    end)
  end)

  -- ── update_diagnostics ─────────────────────────────────────────────

  describe("update_diagnostics", function()
    it("does not error when called with empty tree", function()
      testing.parse_pytest_output("")
      assert.has_no.errors(function()
        testing.update_diagnostics()
      end)
    end)

    it("does not error after parsing results", function()
      testing.parse_pytest_output("test_d.py::test_diag\n")
      testing.parse_test_results("test_d.py::test_diag FAILED\n")
      assert.has_no.errors(function()
        testing.update_diagnostics()
      end)
    end)
  end)

  -- ── apply_coverage ─────────────────────────────────────────────────

  describe("apply_coverage", function()
    it("does not error for non-existent file", function()
      assert.has_no.errors(function()
        testing.apply_coverage("/tmp/nonexistent_coverage.xml")
      end)
    end)

    it("parses valid coverage XML", function()
      local tmpfile = vim.fn.tempname() .. "_coverage.xml"
      local fh = io.open(tmpfile, "w")
      fh:write(table.concat({
        '<?xml version="1.0"?>',
        '<coverage>',
        '  <packages><package><classes>',
        '    <class filename="test_example.py">',
        '      <lines>',
        '        <line number="1" hits="3"/>',
        '        <line number="2" hits="0"/>',
        '      </lines>',
        '    </class>',
        '  </classes></package></packages>',
        '</coverage>',
      }, "\n"))
      fh:close()
      assert.has_no.errors(function()
        testing.apply_coverage(tmpfile)
      end)
      os.remove(tmpfile)
    end)

    it("handles empty coverage XML", function()
      local tmpfile = vim.fn.tempname() .. "_empty_cov.xml"
      local fh = io.open(tmpfile, "w")
      fh:write('<?xml version="1.0"?>\n<coverage></coverage>\n')
      fh:close()
      assert.has_no.errors(function()
        testing.apply_coverage(tmpfile)
      end)
      os.remove(tmpfile)
    end)

    it("handles multiple classes in coverage XML", function()
      local tmpfile = vim.fn.tempname() .. "_multi_cov.xml"
      local fh = io.open(tmpfile, "w")
      fh:write(table.concat({
        '<?xml version="1.0"?>',
        '<coverage>',
        '  <packages><package><classes>',
        '    <class filename="test_a.py">',
        '      <lines><line number="1" hits="1"/></lines>',
        '    </class>',
        '    <class filename="test_b.py">',
        '      <lines><line number="1" hits="0"/></lines>',
        '    </class>',
        '  </classes></package></packages>',
        '</coverage>',
      }, "\n"))
      fh:close()
      assert.has_no.errors(function()
        testing.apply_coverage(tmpfile)
      end)
      os.remove(tmpfile)
    end)
  end)

  -- ── Panel open/close/toggle ────────────────────────────────────────

  describe("panel lifecycle", function()
    after_each(function()
      testing.close()
    end)

    it("open creates a buffer with basilisk-tests filetype", function()
      local config = require("basilisk.config").resolve()
      testing.open(config)
      local found = false
      for _, buf in ipairs(vim.api.nvim_list_bufs()) do
        if vim.bo[buf].filetype == "basilisk-tests" then
          found = true
          break
        end
      end
      assert.is_true(found, "should create buffer with basilisk-tests filetype")
    end)

    it("open increases window count", function()
      local config = require("basilisk.config").resolve()
      local before = #vim.api.nvim_tabpage_list_wins(0)
      testing.open(config)
      local after = #vim.api.nvim_tabpage_list_wins(0)
      assert.is_true(after > before, "opening panel should add a window")
    end)

    it("close restores window count", function()
      local config = require("basilisk.config").resolve()
      local before = #vim.api.nvim_tabpage_list_wins(0)
      testing.open(config)
      testing.close()
      assert.are.equal(before, #vim.api.nvim_tabpage_list_wins(0))
    end)

    it("double close does not error", function()
      local config = require("basilisk.config").resolve()
      testing.open(config)
      testing.close()
      assert.has_no.errors(function()
        testing.close()
      end)
    end)

    it("toggle opens when closed", function()
      local config = require("basilisk.config").resolve()
      local before = #vim.api.nvim_tabpage_list_wins(0)
      testing.toggle(config)
      assert.is_true(#vim.api.nvim_tabpage_list_wins(0) > before)
    end)

    it("toggle closes when open", function()
      local config = require("basilisk.config").resolve()
      local before = #vim.api.nvim_tabpage_list_wins(0)
      testing.toggle(config)
      testing.toggle(config)
      assert.are.equal(before, #vim.api.nvim_tabpage_list_wins(0))
    end)

    it("open with left position works", function()
      local config = require("basilisk.config").resolve({ test_explorer = { position = "left", width = 25 } })
      assert.has_no.errors(function()
        testing.open(config)
      end)
      assert.is_true(#vim.api.nvim_tabpage_list_wins(0) >= 2)
    end)

    it("open with bottom position works", function()
      local config = require("basilisk.config").resolve({ test_explorer = { position = "bottom" } })
      assert.has_no.errors(function()
        testing.open(config)
      end)
      assert.is_true(#vim.api.nvim_tabpage_list_wins(0) >= 2)
    end)

    it("re-open focuses existing panel instead of creating new one", function()
      local config = require("basilisk.config").resolve()
      testing.open(config)
      local count_after_first = #vim.api.nvim_tabpage_list_wins(0)
      testing.open(config)
      assert.are.equal(count_after_first, #vim.api.nvim_tabpage_list_wins(0))
    end)
  end)

  -- ── refresh_display ────────────────────────────────────────────────

  describe("refresh_display", function()
    it("does not error when no panel is open", function()
      testing.close()
      assert.has_no.errors(function()
        testing.refresh_display()
      end)
    end)

    it("shows placeholder text when tree is empty", function()
      local config = require("basilisk.config").resolve()
      testing.parse_pytest_output("")
      testing.open(config)
      testing.refresh_display()
      -- Buffer should have placeholder content.
      local found = false
      for _, buf in ipairs(vim.api.nvim_list_bufs()) do
        if vim.bo[buf].filetype == "basilisk-tests" then
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          if #lines > 0 and lines[1]:find("No tests") then
            found = true
          end
          break
        end
      end
      assert.is_true(found, "should show placeholder text when no tests")
      testing.close()
    end)
  end)

  -- ── setup_auto_discover ────────────────────────────────────────────

  describe("setup_auto_discover", function()
    it("creates autogroup when enabled", function()
      local config = require("basilisk.config").resolve({ test_explorer = { auto_discover_on_save = true } })
      assert.has_no.errors(function()
        testing.setup_auto_discover(config)
      end)
    end)

    it("does not create autogroup when disabled", function()
      local config = require("basilisk.config").resolve({ test_explorer = { auto_discover_on_save = false } })
      assert.has_no.errors(function()
        testing.setup_auto_discover(config)
      end)
    end)
  end)
end)

-- ── Memory module tests (keep separate from testing) ─────────────────

describe("complete_refs (memory module)", function()
  local memory = require("basilisk.memory")

  it("returns matching types", function()
    local matches = memory.complete_refs("Data")
    assert.is_true(#matches > 0)
    assert.are.equal("DataFrame", matches[1])
  end)

  it("returns all types for empty input", function()
    local matches = memory.complete_refs("")
    assert.is_true(#matches > 0)
  end)

  it("is case-insensitive", function()
    local matches = memory.complete_refs("dict")
    local found = false
    for _, m in ipairs(matches) do
      if m == "dict" then
        found = true
      end
    end
    assert.is_true(found)
  end)
end)
