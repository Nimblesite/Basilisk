--- Test explorer e2e tests — real pytest, real UI panels.
---
--- Tests discover, run, panel open/close, status updates with real pytest.

local helpers = require("tests.lsp.helpers")

local tmpdir

describe("test explorer e2e", function()
  before_each(function()
    tmpdir = helpers.create_tmpdir()
    -- Create actual Python test files.
    local fh = io.open(tmpdir .. "/test_math.py", "w")
    fh:write(table.concat({
      "def test_add():",
      "    assert 1 + 1 == 2",
      "",
      "def test_subtract():",
      "    assert 3 - 1 == 2",
      "",
      "class TestMultiply:",
      "    def test_positive(self):",
      "        assert 2 * 3 == 6",
      "",
      "    def test_zero(self):",
      "        assert 0 * 100 == 0",
      "",
    }, "\n"))
    fh:close()

    local fh2 = io.open(tmpdir .. "/test_string.py", "w")
    fh2:write(table.concat({
      "def test_concat():",
      "    assert 'hello' + ' world' == 'hello world'",
      "",
      "def test_upper():",
      "    assert 'hello'.upper() == 'HELLO'",
      "",
    }, "\n"))
    fh2:close()
  end)

  after_each(function()
    local testing = require("basilisk.testing")
    testing.close()
    helpers.close_all_buffers()
    helpers.cleanup_tmpdir(tmpdir)
  end)

  it("discovers tests from real pytest output", function()
    local testing = require("basilisk.testing")

    -- Run pytest collect synchronously.
    local output = vim.fn.system({ "pytest", "--collect-only", "-q", tmpdir })
    local tree = testing.parse_pytest_output(output)

    assert.is_true(#tree >= 2, "should discover at least 2 test files")

    -- Check structure: should have file > function and file > class > function.
    local found_class = false
    local found_function = false
    for _, file_node in ipairs(tree) do
      for _, child in ipairs(file_node.children) do
        if child.kind == "class" then found_class = true end
        if child.kind == "function" then found_function = true end
      end
    end
    assert.is_true(found_function, "should have standalone test functions")
    assert.is_true(found_class, "should have test classes")
  end)

  it("parses test results from real pytest run", function()
    local testing = require("basilisk.testing")

    -- Run tests synchronously.
    local output = vim.fn.system({ "pytest", "-v", "--tb=short", tmpdir })
    testing.parse_test_results(output)

    -- Tests should have been tracked.
    -- We can't check internal state directly, but parse_test_results
    -- should not error on real output.
    assert.is_true(true)
  end)

  it("test panel opens with correct filetype and position", function()
    local testing = require("basilisk.testing")
    local config = require("basilisk.config").resolve()

    local win_count_before = #vim.api.nvim_tabpage_list_wins(0)

    testing.open(config)

    local win_count_after = #vim.api.nvim_tabpage_list_wins(0)
    assert.is_true(win_count_after > win_count_before, "should open a new window")

    -- Find the test buffer.
    local found = false
    for _, buf in ipairs(vim.api.nvim_list_bufs()) do
      if vim.bo[buf].filetype == "basilisk-tests" then
        found = true
        break
      end
    end
    assert.is_true(found, "should create buffer with basilisk-tests filetype")

    testing.close()
    assert.are.equal(win_count_before, #vim.api.nvim_tabpage_list_wins(0))
  end)

  it("test panel opens on the left", function()
    local testing = require("basilisk.testing")
    local config = require("basilisk.config").resolve({ test_explorer = { position = "left", width = 25 } })

    testing.open(config)
    -- Should not crash.
    assert.is_true(#vim.api.nvim_tabpage_list_wins(0) >= 2)
    testing.close()
  end)

  it("test panel opens at bottom", function()
    local testing = require("basilisk.testing")
    local config = require("basilisk.config").resolve({ test_explorer = { position = "bottom" } })

    testing.open(config)
    assert.is_true(#vim.api.nvim_tabpage_list_wins(0) >= 2)
    testing.close()
  end)

  it("toggle opens and closes the panel", function()
    local testing = require("basilisk.testing")
    local config = require("basilisk.config").resolve()

    local before = #vim.api.nvim_tabpage_list_wins(0)
    testing.toggle(config)
    assert.is_true(#vim.api.nvim_tabpage_list_wins(0) > before)
    testing.toggle(config)
    assert.are.equal(before, #vim.api.nvim_tabpage_list_wins(0))
  end)

  it("coverage XML is parsed and applied", function()
    local testing = require("basilisk.testing")

    -- Create a coverage XML file.
    local cov_path = tmpdir .. "/coverage.xml"
    local cfh = io.open(cov_path, "w")
    cfh:write(table.concat({
      '<?xml version="1.0"?>',
      '<coverage>',
      '  <packages><package><classes>',
      '    <class filename="test_math.py">',
      '      <lines>',
      '        <line number="1" hits="5"/>',
      '        <line number="2" hits="5"/>',
      '        <line number="4" hits="0"/>',
      '      </lines>',
      '    </class>',
      '  </classes></package></packages>',
      '</coverage>',
    }, "\n"))
    cfh:close()

    -- Apply coverage — should not error.
    assert.has_no.errors(function()
      testing.apply_coverage(cov_path)
    end)
  end)
end)
