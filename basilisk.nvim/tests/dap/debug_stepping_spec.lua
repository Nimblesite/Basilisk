--- DAP stepping E2E tests — per-function fixture coverage.
---
--- Matches VS Code debug-integration.test.ts tests 6-13:
---   list_ops, dict_ops, nested_call (step into/out), loop_and_accumulate,
---   conditional_branches, type_variety, class_instance, scopes enumeration.

local lsp_helpers = require("tests.lsp.helpers")
local dap_helpers = require("tests.dap.helpers")

local binary = lsp_helpers.find_binary()
if not binary then
  describe("DAP stepping (SKIPPED — no binary)", function()
    it("skipped", function()
      pending("basilisk binary not found")
    end)
  end)
  return
end

local dap_ok, dap = pcall(require, "dap")
if not dap_ok or not dap_helpers.is_debugpy_installed() or not dap_helpers.fixture_path() then
  describe("DAP stepping (SKIPPED — missing deps)", function()
    it("skipped", function()
      pending("nvim-dap, debugpy, or fixture missing")
    end)
  end)
  return
end

vim.ui.select = function(items, _, on_choice)
  on_choice(items[1], 1)
end

local tmpdir

--- Launch fixture, set breakpoint, wait for stop.
---@param line integer
---@return boolean stopped
local function launch_at(line)
  local filepath = tmpdir .. "/debug_stepping.py"
  vim.cmd("edit " .. vim.fn.fnameescape(filepath))
  local buf = vim.api.nvim_get_current_buf()
  lsp_helpers.wait_for_server_ready(buf)
  vim.api.nvim_win_set_cursor(0, { line, 0 })
  dap.toggle_breakpoint()
  dap.run({
    type = "basilisk",
    request = "launch",
    name = "Test",
    program = filepath,
    justMyCode = true,
  })
  return dap_helpers.wait_for_stopped()
end

describe("DAP stepping", function()
  before_each(function()
    tmpdir = lsp_helpers.create_tmpdir()
    local fh = io.open(tmpdir .. "/pyproject.toml", "w")
    assert(fh)
    fh:write('[project]\nname = "test"\nversion = "0.1.0"\n')
    fh:close()
    local src = io.open(dap_helpers.fixture_path(), "r")
    assert(src)
    local content = src:read("*a")
    src:close()
    local dst = io.open(tmpdir .. "/debug_stepping.py", "w")
    assert(dst)
    dst:write(content)
    dst:close()
    vim.lsp.config("basilisk", {
      cmd = { binary, "lsp" },
      filetypes = { "python" },
      root_markers = { "pyproject.toml", ".git" },
    })
    vim.lsp.enable("basilisk")
    require("basilisk.dap").setup({ debugger = { enabled = true }, python = "python3" })
  end)

  after_each(function()
    dap_helpers.cleanup_session()
    lsp_helpers.stop_clients()
    lsp_helpers.close_all_buffers()
    dap.clear_breakpoints()
    lsp_helpers.cleanup_tmpdir(tmpdir)
  end)

  -- ── list_ops: step through and assert list contents ─────────────────

  it("list_ops: step through list mutations", function()
    assert.is_true(launch_at(31))

    -- Step: items = [1, 2, 3]
    dap_helpers.step_and_wait("next")
    local vars = dap_helpers.get_local_variables()
    assert.is_not_nil(vars["items"])

    -- Step: items.append(4)
    dap_helpers.step_and_wait("next")
    local result = dap_helpers.evaluate("len(items)")
    assert.are.equal("4", result)

    -- Step: items.insert(0, 0)
    dap_helpers.step_and_wait("next")
    result = dap_helpers.evaluate("items[0]")
    assert.are.equal("0", result)

    -- Step: total = sum(items)
    dap_helpers.step_and_wait("next")
    vars = dap_helpers.get_local_variables()
    assert.are.equal("10", vars["total"])

    -- Step: count = len(items)
    dap_helpers.step_and_wait("next")
    vars = dap_helpers.get_local_variables()
    assert.are.equal("5", vars["count"])
  end)

  -- ── dict_ops: step through and assert dict contents ─────────────────

  it("dict_ops: step through dictionary operations", function()
    assert.is_true(launch_at(41))

    -- Step: data = {"a": 1, "b": 2}
    dap_helpers.step_and_wait("next")
    assert.are.equal("2", dap_helpers.evaluate("len(data)"))
    assert.are.equal("1", dap_helpers.evaluate('data["a"]'))

    -- Step: data["c"] = 3
    dap_helpers.step_and_wait("next")
    assert.are.equal("3", dap_helpers.evaluate("len(data)"))
    assert.are.equal("3", dap_helpers.evaluate('data["c"]'))

    -- Step: keys = list(data.keys())
    dap_helpers.step_and_wait("next")
    assert.are.equal("3", dap_helpers.evaluate("len(keys)"))

    -- Step: total = sum(data.values())
    dap_helpers.step_and_wait("next")
    local vars = dap_helpers.get_local_variables()
    assert.are.equal("6", vars["total"])

    -- Step: has_a = "a" in data
    dap_helpers.step_and_wait("next")
    vars = dap_helpers.get_local_variables()
    assert.are.equal("True", vars["has_a"])
  end)

  -- ── nested_call: step into/out ──────────────────────────────────────

  it("nested_call: step into function and back out", function()
    assert.is_true(launch_at(51))

    -- Step: a = 5
    dap_helpers.step_and_wait("next")
    local vars = dap_helpers.get_local_variables()
    assert.are.equal("5", vars["a"])

    -- Step INTO: b = double(a) → enter double()
    dap_helpers.step_and_wait("stepIn")
    local frames = dap_helpers.get_stack_trace()
    assert.are.equal("double", frames[1].name)
    vars = dap_helpers.get_local_variables()
    assert.are.equal("5", vars["n"])

    -- Step over inside double: result = n * 2
    dap_helpers.step_and_wait("next")
    vars = dap_helpers.get_local_variables()
    assert.are.equal("10", vars["result"])

    -- Step OUT back to nested_call
    dap_helpers.step_and_wait("stepOut")
    frames = dap_helpers.get_stack_trace()
    assert.are.equal("nested_call", frames[1].name)
    -- After stepOut, we land on the line where b = double(a) completes.
    -- The variable may need one more step to be assigned.
    dap_helpers.step_and_wait("next")
    vars = dap_helpers.get_local_variables()
    assert.are.equal("10", vars["b"])
  end)

  -- ── loop_and_accumulate: verify accumulator ─────────────────────────

  it("loop_and_accumulate: verify accumulator at iterations", function()
    assert.is_true(launch_at(65))

    -- Step: total = 0
    dap_helpers.step_and_wait("next")
    assert.are.equal("0", dap_helpers.evaluate("total"))

    -- Step into for loop header
    dap_helpers.step_and_wait("next")

    -- i=0: total += 0 → 0
    dap_helpers.step_and_wait("next")
    assert.are.equal("0", dap_helpers.evaluate("total"))

    -- i=1: for header + body → total = 1
    dap_helpers.step_and_wait("next")
    dap_helpers.step_and_wait("next")
    assert.are.equal("1", dap_helpers.evaluate("total"))

    -- i=2: → total = 3
    dap_helpers.step_and_wait("next")
    dap_helpers.step_and_wait("next")
    assert.are.equal("3", dap_helpers.evaluate("total"))

    -- i=3: → total = 6
    dap_helpers.step_and_wait("next")
    dap_helpers.step_and_wait("next")
    assert.are.equal("6", dap_helpers.evaluate("total"))

    -- i=4: → total = 10
    dap_helpers.step_and_wait("next")
    dap_helpers.step_and_wait("next")
    assert.are.equal("10", dap_helpers.evaluate("total"))
  end)

  -- ── conditional_branches: verify correct branch ─────────────────────

  it("conditional_branches: verifies elif branch taken", function()
    -- Break at line 81 (return label) — after branch is resolved.
    assert.is_true(launch_at(81))

    local vars = dap_helpers.get_local_variables()
    assert.are.equal("42", vars["x"])
    assert.are.equal("'medium'", vars["label"])
    assert.are.equal("True", dap_helpers.evaluate('label == "medium"'))
    assert.are.equal("True", dap_helpers.evaluate('label != "big"'))
    assert.are.equal("True", dap_helpers.evaluate('label != "small"'))
  end)

  -- ── type_variety: verify Python type representations ────────────────

  it("type_variety: verifies different Python types", function()
    -- Break at line 105 (return an_int) — all vars set.
    assert.is_true(launch_at(105))

    local vars = dap_helpers.get_local_variables()
    assert.are.equal("42", vars["an_int"])
    assert.are.equal("3.14", vars["a_float"])
    assert.are.equal("True", vars["a_bool"])
    assert.are.equal("None", vars["a_none"])

    assert.are.equal("'int'", dap_helpers.evaluate("type(an_int).__name__"))
    assert.are.equal("'float'", dap_helpers.evaluate("type(a_float).__name__"))
    assert.are.equal("'bool'", dap_helpers.evaluate("type(a_bool).__name__"))
    assert.are.equal("True", dap_helpers.evaluate("a_none is None"))
    assert.are.equal("3", dap_helpers.evaluate("len(a_tuple)"))
    assert.are.equal("3", dap_helpers.evaluate("len(a_set)"))
    assert.are.equal("'bytes'", dap_helpers.evaluate("type(a_bytes).__name__"))
  end)

  -- ── class_instance: object attributes and method calls ──────────────

  it("class_instance: inspect object attributes and method result", function()
    assert.is_true(launch_at(119))

    -- Step: p = Point(3, 4)
    dap_helpers.step_and_wait("next")
    assert.are.equal("3", dap_helpers.evaluate("p.x"))
    assert.are.equal("4", dap_helpers.evaluate("p.y"))
    assert.are.equal("'Point'", dap_helpers.evaluate("type(p).__name__"))

    -- Step: mag = p.magnitude()
    dap_helpers.step_and_wait("next")
    local vars = dap_helpers.get_local_variables()
    assert.are.equal("5.0", vars["mag"])
    assert.are.equal("True", dap_helpers.evaluate("mag == 5.0"))
    assert.are.equal("25", dap_helpers.evaluate("p.x ** 2 + p.y ** 2"))
  end)

  -- ── scopes: verify locals scope enumeration ─────────────────────────

  it("scopes: Locals scope has correct variables", function()
    -- Break at line 13 (z = x + y) — x and y are set.
    assert.is_true(launch_at(13))

    local vars = dap_helpers.get_local_variables()
    assert.are.equal("10", vars["x"])
    assert.are.equal("20", vars["y"])
    -- z is not yet set (we're AT line 13, not past it).
    assert.is_nil(vars["z"])
  end)

  -- ── watch: complex expression evaluation ────────────────────────────

  it("watch: evaluates complex expressions at breakpoint", function()
    -- Stop at line 15 in arithmetic where x=10, y=20, z=30, w=60.
    assert.is_true(launch_at(15))

    -- Arithmetic
    assert.are.equal("60", dap_helpers.evaluate("x + y + z"))
    assert.are.equal("6", dap_helpers.evaluate("w // x"))
    assert.are.equal("4", dap_helpers.evaluate("w % 7"))
    assert.are.equal("60", dap_helpers.evaluate("abs(-w)"))
    assert.are.equal("10", dap_helpers.evaluate("min(x, y, z, w)"))
    assert.are.equal("60", dap_helpers.evaluate("max(x, y, z, w)"))

    -- Boolean
    assert.are.equal("True", dap_helpers.evaluate("x < y"))
    assert.are.equal("True", dap_helpers.evaluate("z == x + y"))
    assert.are.equal("True", dap_helpers.evaluate("w == z * 2"))

    -- Type checking
    assert.are.equal("True", dap_helpers.evaluate("isinstance(x, int)"))
    assert.are.equal("False", dap_helpers.evaluate("isinstance(x, str)"))

    -- String formatting
    assert.are.equal("'10 + 20 = 30'", dap_helpers.evaluate('f"{x} + {y} = {z}"'))

    -- List comprehension
    assert.are.equal("[20, 40, 60]", dap_helpers.evaluate("[v * 2 for v in [x, y, z]]"))
  end)

  -- ── REPL: debug console evaluation ──────────────────────────────────

  it("REPL: evaluates expressions in debug console context", function()
    assert.is_true(launch_at(13))

    assert.are.equal("30", dap_helpers.evaluate("x + y"))
    assert.are.equal("[10, 20]", dap_helpers.evaluate("[x, y]"))

    local dict_result = dap_helpers.evaluate("dict(a=x, b=y)")
    assert.is_not_nil(dict_result)
    assert.is_truthy(dict_result:find("a"))
    assert.is_truthy(dict_result:find("b"))
  end)
end)
