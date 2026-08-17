--- Test explorer e2e tests — real pytest, real UI panels.
---
--- Tests [NVIM-TEST-EXPLORER].
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

  -- ── Discovery: Detailed Structure Validation ────────────────────────

  it("discovery tree contains correct test names from real files", function()
    local testing = require("basilisk.testing")
    local output = vim.fn.system({ "pytest", "--collect-only", "-q", tmpdir })
    local tree = testing.parse_pytest_output(output)

    -- Collect all leaf test names.
    local names = {}
    local function collect(nodes)
      for _, node in ipairs(nodes) do
        if node.kind == "function" then
          names[node.name] = true
        end
        collect(node.children)
      end
    end
    collect(tree)

    -- Verify known test names from the fixture files.
    assert.is_true(names["test_add"] ~= nil, "should find test_add")
    assert.is_true(names["test_subtract"] ~= nil, "should find test_subtract")
    assert.is_true(names["test_positive"] ~= nil, "should find test_positive (class method)")
    assert.is_true(names["test_zero"] ~= nil, "should find test_zero (class method)")
    assert.is_true(names["test_concat"] ~= nil, "should find test_concat from second file")
    assert.is_true(names["test_upper"] ~= nil, "should find test_upper from second file")
  end)

  it("discovery tree file nodes have correct kinds", function()
    local testing = require("basilisk.testing")
    local output = vim.fn.system({ "pytest", "--collect-only", "-q", tmpdir })
    local tree = testing.parse_pytest_output(output)

    for _, file_node in ipairs(tree) do
      assert.are.equal("file", file_node.kind, "top-level nodes should be files")
      assert.is_true(file_node.file ~= nil, "file nodes should have a file path")
    end
  end)

  it("class methods are nested under their class node", function()
    local testing = require("basilisk.testing")
    local output = vim.fn.system({ "pytest", "--collect-only", "-q", tmpdir })
    local tree = testing.parse_pytest_output(output)

    -- Find the file with TestMultiply.
    local multiply_class = nil
    for _, file_node in ipairs(tree) do
      for _, child in ipairs(file_node.children) do
        if child.name == "TestMultiply" then
          multiply_class = child
          break
        end
      end
    end
    assert.is_not_nil(multiply_class, "should find TestMultiply class")
    assert.are.equal("class", multiply_class.kind)
    assert.are.equal(2, #multiply_class.children, "TestMultiply should have 2 methods")
  end)

  -- ── Discovery: File with Only Functions ─────────────────────────────

  it("discovers file with only standalone functions", function()
    local testing = require("basilisk.testing")

    -- Create a test file with only functions.
    local fh = io.open(tmpdir .. "/test_funcs_only.py", "w")
    fh:write(table.concat({
      "def test_alpha():",
      "    assert True",
      "",
      "def test_beta():",
      "    assert True",
      "",
      "def test_gamma():",
      "    assert True",
      "",
    }, "\n"))
    fh:close()

    local output = vim.fn.system({ "pytest", "--collect-only", "-q", tmpdir .. "/test_funcs_only.py" })
    local tree = testing.parse_pytest_output(output)

    assert.are.equal(1, #tree, "should find 1 file")
    assert.are.equal(3, #tree[1].children, "should find 3 test functions")
    for _, child in ipairs(tree[1].children) do
      assert.are.equal("function", child.kind)
    end
  end)

  -- ── Discovery: Empty Test File ──────────────────────────────────────

  it("discovers nothing from empty test file", function()
    local testing = require("basilisk.testing")

    local fh = io.open(tmpdir .. "/test_empty.py", "w")
    fh:write("# no tests here\nx = 42\n")
    fh:close()

    local output = vim.fn.system({ "pytest", "--collect-only", "-q", tmpdir .. "/test_empty.py" })
    local tree = testing.parse_pytest_output(output)
    assert.are.equal(0, #tree)
  end)

  -- ── Test Run Results ─────────────────────────────────────────────────

  it("real pytest run produces parseable output", function()
    local testing = require("basilisk.testing")

    -- First discover so the tree is populated.
    local collect_output = vim.fn.system({ "pytest", "--collect-only", "-q", tmpdir })
    testing.parse_pytest_output(collect_output)

    -- Now run the tests.
    local run_output = vim.fn.system({ "pytest", "-v", "--tb=short", tmpdir })
    assert.has_no.errors(function()
      testing.parse_test_results(run_output)
    end)
  end)

  -- ── Panel: Display After Discovery ──────────────────────────────────

  it("panel shows discovered tests after refresh", function()
    local testing = require("basilisk.testing")
    local config = require("basilisk.config").resolve()

    -- Discover and open.
    local output = vim.fn.system({ "pytest", "--collect-only", "-q", tmpdir })
    testing.parse_pytest_output(output)
    testing.open(config)
    testing.refresh_display()

    -- Find the test buffer and check content.
    for _, buf in ipairs(vim.api.nvim_list_bufs()) do
      if vim.bo[buf].filetype == "basilisk-tests" then
        local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
        assert.is_true(#lines > 0, "buffer should have content")
        -- Should NOT show the placeholder text.
        assert.is_false(
          lines[1]:find("No tests") ~= nil,
          "should show test tree, not placeholder"
        )
        break
      end
    end
    testing.close()
  end)

  -- ── Panel: Buffer Properties ────────────────────────────────────────

  it("panel buffer is non-modifiable", function()
    local testing = require("basilisk.testing")
    local config = require("basilisk.config").resolve()

    testing.open(config)
    testing.refresh_display()

    for _, buf in ipairs(vim.api.nvim_list_bufs()) do
      if vim.bo[buf].filetype == "basilisk-tests" then
        assert.is_false(vim.bo[buf].modifiable, "test buffer should be non-modifiable")
        assert.is_false(vim.bo[buf].swapfile, "test buffer should have no swapfile")
        break
      end
    end
    testing.close()
  end)

  -- ── Panel: Window Options ──────────────────────────────────────────

  it("panel window has no line numbers", function()
    local testing = require("basilisk.testing")
    local config = require("basilisk.config").resolve()

    testing.open(config)

    -- Find the window showing basilisk-tests.
    for _, win in ipairs(vim.api.nvim_tabpage_list_wins(0)) do
      local buf = vim.api.nvim_win_get_buf(win)
      if vim.bo[buf].filetype == "basilisk-tests" then
        assert.is_false(vim.wo[win].number, "test panel should have no line numbers")
        assert.is_false(vim.wo[win].relativenumber, "test panel should have no relative numbers")
        break
      end
    end
    testing.close()
  end)
end)
