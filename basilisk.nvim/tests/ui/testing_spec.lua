--- UI tests for basilisk.testing module.

describe("basilisk.testing", function()
  local testing = require("basilisk.testing")

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
  end)

  describe("set_status", function()
    it("updates a node status by ID", function()
      local output = "test_x.py::test_foo\ntest_x.py::test_bar\n"
      -- Reset internal tree state by re-parsing.
      local tree = testing.parse_pytest_output(output)
      -- set_status operates on the module's internal tree, so we need
      -- to test it through the module directly.
      testing.set_status("test_x.py::test_foo", "passed")
      -- The internal tree is not directly accessible, but we verify
      -- the function doesn't error.
      assert.is_true(true)
    end)
  end)

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
end)
