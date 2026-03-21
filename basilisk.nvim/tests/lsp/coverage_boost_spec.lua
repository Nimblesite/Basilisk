--- E2e coverage boost tests exercising uncovered code paths.
---
--- Exercises memory, profiling, testing, binary, log, lsp, init, ui,
--- commands, statusline, tab_tracking, config, and health modules
--- through REAL interactions — no mocking.

local helpers = require("tests.lsp.helpers")

local binary = helpers.find_binary()
if not binary then
  describe("coverage boost (SKIPPED — no binary)", function()
    it("skipped", function()
      pending("basilisk binary not found")
    end)
  end)
  return
end

-- ── Helpers ──────────────────────────────────────────────────────────────────

local function close_floats()
  for _, w in ipairs(vim.api.nvim_list_wins()) do
    local cfg = vim.api.nvim_win_get_config(w)
    if cfg.relative and cfg.relative ~= "" then
      pcall(vim.api.nvim_win_close, w, true)
    end
  end
end

local tmpdir

-- ── Tests ────────────────────────────────────────────────────────────────────

describe("coverage boost e2e", function()
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
    pcall(function() require("basilisk.testing").close() end)
    helpers.stop_clients()
    helpers.close_all_buffers()
    helpers.cleanup_tmpdir(tmpdir)
  end)

  -- ── ui.lua ──────────────────────────────────────────────────────────────

  it("ui.open_float creates float with q keymap", function()
    local ui = require("basilisk.ui")
    local buf, win = ui.open_float("Test Title", { "line 1", "line 2" })

    assert.is_true(vim.api.nvim_win_is_valid(win))
    assert.is_true(vim.api.nvim_buf_is_valid(buf))

    local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
    assert.are.equal("line 1", lines[1])
    assert.are.equal("line 2", lines[2])
    assert.is_false(vim.bo[buf].modifiable)

    vim.api.nvim_set_current_win(win)
    vim.api.nvim_feedkeys("q", "x", false)
    vim.wait(200)
    assert.is_false(vim.api.nvim_win_is_valid(win))
  end)

  it("ui.get_client returns nil when no LSP", function()
    helpers.stop_clients()
    local ui = require("basilisk.ui")
    assert.is_nil(ui.get_client())
  end)

  it("ui.get_client returns client when LSP active", function()
    local buf = helpers.open_python_file(tmpdir, "test_ui_client.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)
    local ui = require("basilisk.ui")
    assert.is_not_nil(ui.get_client())
  end)

  -- ── log.lua ─────────────────────────────────────────────────────────────

  it("log set_level + all log levels + file logging", function()
    local log = require("basilisk.log")

    for _, lvl in ipairs({ "trace", "debug", "info", "warn", "error" }) do
      log.set_level(lvl)
      log.trace("t %s", "a")
      log.debug("d %d", 1)
      log.info("i")
      log.warn("w")
      log.error("e")
    end

    log.set_level("invalid")

    local tmplog = vim.fn.tempname() .. ".log"
    log.enable_file(tmplog)
    log.set_level("info")
    log.info("file test")
    log.close_file()
    log.close_file()

    local fh = io.open(tmplog, "r")
    assert.is_not_nil(fh)
    local content = fh:read("*a")
    fh:close()
    assert.truthy(content:find("file test"))
    os.remove(tmplog)

    log.set_level("info")
  end)

  -- ── binary.lua ──────────────────────────────────────────────────────────

  it("binary resolve cascade", function()
    local bin_mod = require("basilisk.binary")

    bin_mod.resolve(nil)
    bin_mod.resolve("")
    bin_mod.resolve("/nonexistent/basilisk")

    local ls_path = vim.fn.exepath("ls")
    if ls_path ~= "" then
      assert.is_not_nil(bin_mod.resolve(ls_path))
    end

    local orig = vim.env.BASILISK_PATH
    vim.env.BASILISK_PATH = nil
    bin_mod.resolve()
    vim.env.BASILISK_PATH = ""
    bin_mod.resolve()
    vim.env.BASILISK_PATH = "/nonexistent"
    bin_mod.resolve()
    if ls_path ~= "" then
      vim.env.BASILISK_PATH = ls_path
      assert.is_not_nil(bin_mod.resolve())
    end
    vim.env.BASILISK_PATH = orig

    assert.is_nil(bin_mod.version("/nonexistent"))
    if ls_path ~= "" then
      bin_mod.version(ls_path)
    end
  end)

  -- ── config.lua ──────────────────────────────────────────────────────────

  it("config resolve and validate", function()
    local config_mod = require("basilisk.config")

    local d = config_mod.defaults
    assert.are.equal("wholeModule", d.analysis_mode)
    assert.is_true(d.enabled)

    config_mod.resolve()
    config_mod.resolve({})
    config_mod.resolve({ analysis_mode = "openFilesOnly" })
    config_mod.resolve({ ruff = { enabled = false } })
    config_mod.resolve({ inlay_hints = { parameter_names = false } })
    config_mod.resolve({ debugger = { type_checking = true } })
    config_mod.resolve({ uv = { auto_sync = true } })
    config_mod.resolve({ test_explorer = { position = "left", width = 30 } })
    config_mod.resolve({ test_explorer = { position = "bottom" } })

    assert.are.equal(0, #config_mod.validate(config_mod.resolve()))
    assert.are.equal(1, #config_mod.validate(config_mod.resolve({ analysis_mode = "bad" })))
    assert.are.equal(1, #config_mod.validate(config_mod.resolve({ test_explorer = { framework = "bad" } })))
    assert.are.equal(1, #config_mod.validate(config_mod.resolve({ test_explorer = { position = "top" } })))
    assert.are.equal(1, #config_mod.validate(config_mod.resolve({ log_level = "verbose" })))
  end)

  -- ── statusline.lua ──────────────────────────────────────────────────────

  it("statusline all states + diagnostics", function()
    local sl = require("basilisk.statusline")

    for _, state in ipairs({ "stopped", "starting", "error", "ready" }) do
      sl.set_state(state)
      assert.truthy(sl.get():find("Basilisk"))
      sl.get_color()
    end

    sl.set_state("stopped")
    assert.are.equal("Comment", sl.get_color())
    sl.set_state("starting")
    assert.are.equal("DiagnosticWarn", sl.get_color())
    sl.set_state("error")
    assert.are.equal("DiagnosticError", sl.get_color())

    sl.set_state("ready")
    sl.update()
    sl.get()
    sl.get_color()

    assert.are.equal("string", type(sl.lualine_component[1]()))
    sl.lualine_component.color()
  end)

  -- ── memory.lua ──────────────────────────────────────────────────────────

  it("memory display_leak_report + display_retention_paths + complete_refs", function()
    local mem = require("basilisk.memory")

    mem.display_leak_report(nil); close_floats()
    mem.display_leak_report({ leaks = {} }); close_floats()
    mem.display_leak_report({
      leaks = {
        { typeName = "DataFrame", count = 15, totalSize = "1.2MB",
          location = { file = "/tmp/t.py", line = 42 } },
        { typeName = "dict", count = 100, totalSize = "500KB" },
      },
    }); close_floats()

    mem.display_retention_paths("dict", nil); close_floats()
    mem.display_retention_paths("DataFrame", { retentionPaths = {} }); close_floats()
    mem.display_retention_paths("DataFrame", {
      retentionPaths = { {
        confidence = 0.85,
        steps = {
          { name = "cache", kind = "variable" },
          { name = "__dict__", kind = "attribute" },
        },
      } },
    }); close_floats()

    assert.is_true(#mem.complete_refs("") > 0)
    assert.are.equal("DataFrame", mem.complete_refs("Data")[1])
    assert.are.equal(0, #mem.complete_refs("nonexistent_xyz"))
    assert.is_true(#mem.complete_refs("tensor") > 0)
  end)

  it("memory start/stop/refs with real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_mem.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local mem = require("basilisk.memory")
    mem.start()
    vim.wait(500)
    mem.stop()
    vim.wait(500)
    mem.refs("dict")
    vim.wait(500)
    close_floats()
  end)

  it("memory start/stop/refs without client", function()
    helpers.stop_clients()
    local mem = require("basilisk.memory")
    mem.start()
    mem.stop()
    mem.refs("dict")
  end)

  -- ── profiling.lua ───────────────────────────────────────────────────────

  it("profiling display + heat map + flamegraph", function()
    local prof = require("basilisk.profiling")

    prof.display_results(nil); close_floats()
    prof.display_results({ hotFunctions = {} }); close_floats()
    prof.display_results({
      hotFunctions = {
        { name = "hot", file = "/tmp/test.py", line = 10, percentage = 55 },
        { name = "warm", file = "/tmp/test.py", line = 25, percentage = 25 },
        { name = "cool", file = "/tmp/test.py", line = 40, percentage = 5 },
      },
    }); close_floats()

    prof.apply_heat_map(nil)
    prof.apply_heat_map({})
    prof.apply_heat_map({ { name = "x", file = "/nonexistent.py", line = 1, percentage = 60 } })

    prof.export_flamegraph(nil)
    prof.export_flamegraph({})
    prof.export_flamegraph({ speedscopeJson = '{"version":"0.0.1"}' })
  end)

  it("profiling start/stop/snapshot with real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_prof.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local prof = require("basilisk.profiling")
    prof.start()
    vim.wait(500)
    prof.start(1234)
    vim.wait(500)
    prof.stop()
    vim.wait(500)
    prof.snapshot()
    vim.wait(500)
    close_floats()
  end)

  it("profiling without client", function()
    helpers.stop_clients()
    local prof = require("basilisk.profiling")
    prof.start()
    prof.stop()
    prof.snapshot()
  end)

  -- ── lsp.lua ─────────────────────────────────────────────────────────────

  it("lsp start + restart + backoff", function()
    local lsp_mod = require("basilisk.lsp")
    local config_mod = require("basilisk.config")

    assert.is_true(lsp_mod.get_restart_count() >= 0)
    lsp_mod.reset_restart_count()
    assert.are.equal(0, lsp_mod.get_restart_count())

    local no_bin_config = config_mod.resolve({ binary_path = "/nonexistent" })
    assert.is_false(lsp_mod.start(no_bin_config))

    local real_config = config_mod.resolve({ binary_path = binary })
    assert.is_true(lsp_mod.start(real_config))

    lsp_mod.reset_restart_count()
    lsp_mod.restart(real_config, false)
    vim.wait(200)

    lsp_mod.restart(real_config, true)
    vim.wait(200)

    lsp_mod.reset_restart_count()
    for _ = 1, 4 do
      lsp_mod.restart(real_config, false)
    end
    lsp_mod.restart(real_config, true)
  end)

  -- ── testing.lua ─────────────────────────────────────────────────────────

  it("testing parse + set_status + update_diagnostics + refresh", function()
    local testing = require("basilisk.testing")

    testing.parse_pytest_output("")
    testing.parse_pytest_output("no tests ran\n")
    testing.parse_pytest_output("===== 5 items =====\n")
    testing.parse_pytest_output("test_a.py::test_one\ntest_a.py::test_two\n")
    testing.parse_pytest_output("test_a.py::TestClass::test_method\n")
    testing.parse_pytest_output("test_a.py::TestClass::test_m1\ntest_a.py::TestClass::test_m2\n")

    local tree = testing.parse_pytest_output("test_a.py::test_one\ntest_b.py::test_two\n")
    assert.are.equal(2, #tree)

    testing.set_status("test_a.py::test_one", "passed")
    testing.set_status("test_a.py::test_one", "failed")
    testing.set_status("nonexistent", "passed")
    testing.set_status(nil, "passed")

    testing.parse_test_results("")
    testing.parse_test_results("test_a.py::test_one PASSED\ntest_a.py::test_two FAILED\n")
    testing.update_diagnostics()
    testing.refresh_display()
  end)

  it("testing open/close/toggle for every position", function()
    local testing = require("basilisk.testing")
    local config_mod = require("basilisk.config")

    for _, pos in ipairs({ "right", "left", "bottom" }) do
      testing.open(config_mod.resolve({ test_explorer = { position = pos, width = 30 } }))
      testing.refresh_display()
      testing.close()
    end

    testing.toggle(config_mod.resolve())
    testing.toggle(config_mod.resolve())
  end)

  it("testing setup_auto_discover", function()
    local testing = require("basilisk.testing")
    local config_mod = require("basilisk.config")

    testing.setup_auto_discover(config_mod.resolve({ test_explorer = { auto_discover_on_save = false } }))
    testing.setup_auto_discover(config_mod.resolve({ test_explorer = { auto_discover_on_save = true } }))
  end)

  it("testing discover + run with real pytest", function()
    local testing = require("basilisk.testing")
    local config_mod = require("basilisk.config")

    local test_file = tmpdir .. "/test_example.py"
    local tfh = io.open(test_file, "w")
    tfh:write("def test_pass():\n    assert 1 + 1 == 2\n\ndef test_fail():\n    assert 1 == 2\n")
    tfh:close()

    local cfg = config_mod.resolve()

    testing.open(cfg)

    testing.discover(cfg)
    vim.wait(5000, function() return false end, 100)
    testing.refresh_display()

    testing.run(cfg, test_file .. "::test_pass")
    vim.wait(5000, function() return false end, 100)
    testing.refresh_display()
    testing.update_diagnostics()

    testing.run(cfg)
    vim.wait(3000, function() return false end, 100)

    testing.set_status(test_file .. "::test_pass", "passed")
    testing.set_status(test_file .. "::test_fail", "failed")
    testing.refresh_display()
    testing.update_diagnostics()

    pcall(testing.debug, cfg, test_file .. "::test_pass")

    testing.close()
  end)

  it("testing apply_coverage real XML", function()
    local testing = require("basilisk.testing")

    local cov_xml = tmpdir .. "/coverage.xml"
    local cxfh = io.open(cov_xml, "w")
    cxfh:write([[<?xml version="1.0"?>
<coverage><packages><package><classes>
<class filename="test_example.py">
<lines><line number="1" hits="5"/><line number="2" hits="0"/></lines>
</class></classes></package></packages></coverage>]])
    cxfh:close()

    testing.apply_coverage(cov_xml)
    testing.apply_coverage("/nonexistent/coverage.xml")
  end)

  -- ── init.lua ────────────────────────────────────────────────────────────

  it("init.setup full lifecycle", function()
    package.loaded["basilisk"] = nil
    package.loaded["basilisk.init"] = nil

    local init_mod = require("basilisk")
    init_mod.setup({ binary_path = binary })
    assert.is_not_nil(init_mod.config)

    init_mod.setup({})
  end)

  -- ── commands.lua ────────────────────────────────────────────────────────

  it(":BasiliskRestart force restarts", function()
    local buf = helpers.open_python_file(tmpdir, "test_restart.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    package.loaded["basilisk"] = nil
    package.loaded["basilisk.init"] = nil
    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local ok = pcall(vim.cmd, "BasiliskRestart")
    assert.is_true(ok)
  end)

  it("commands with callbacks wait for response", function()
    local buf = helpers.open_python_file(tmpdir, "test_cb.py", "def greet(name):\n    return name\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    pcall(vim.cmd, "BasiliskFixFile")
    vim.wait(1000)
    pcall(vim.cmd, "BasiliskFixWorkspace")
    vim.wait(1000)
    pcall(vim.cmd, "BasiliskAdoptFile")
    vim.wait(1000)
    pcall(vim.cmd, "BasiliskAdoptWorkspace")
    vim.wait(1000)
    pcall(vim.cmd, "BasiliskUnadoptFile")
    vim.wait(1000)
    pcall(vim.cmd, "BasiliskShowOutput")
    vim.wait(500)
    pcall(vim.cmd, "BasiliskTestDiscover")
    vim.wait(2000)
    close_floats()
  end)

  it("uv commands send to real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_uv.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    for _, cmd in ipairs({ "BasiliskUvSync", "BasiliskUvLock" }) do
      pcall(vim.cmd, cmd)
      vim.wait(1000)
    end

    pcall(vim.cmd, "BasiliskUvAdd requests")
    vim.wait(1000)
    pcall(vim.cmd, "BasiliskUvAddDev pytest")
    vim.wait(1000)
    pcall(vim.cmd, "BasiliskUvRemove requests")
    vim.wait(1000)
    pcall(vim.cmd, "BasiliskUvCreateEnv 3.12")
    vim.wait(1000)
  end)

  it("test and debug commands", function()
    local buf = helpers.open_python_file(tmpdir, "test_cmds.py", "def test_x():\n    pass\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    pcall(vim.cmd, "BasiliskTestRun")
    vim.wait(200)
    pcall(vim.cmd, "BasiliskTestDebug test_foo.py::test_bar")
    vim.wait(200)
    pcall(vim.cmd, "BasiliskDebugFile")
    vim.wait(200)
  end)

  it("commands without LSP client", function()
    helpers.stop_clients()
    vim.wait(500)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    pcall(vim.cmd, "BasiliskOrganizeImports")
    pcall(vim.cmd, "BasiliskFixFile")
    pcall(vim.cmd, "BasiliskInfo")
    vim.wait(200)
    close_floats()
  end)

  -- ── tab_tracking.lua ────────────────────────────────────────────────────

  it("tab tracking all modes + buffer lifecycle", function()
    local tt = require("basilisk.tab_tracking")
    local config_mod = require("basilisk.config")

    tt.setup(config_mod.resolve({ analysis_mode = "wholeModule" }))
    tt.setup(config_mod.resolve({ analysis_mode = "crossModule" }))
    tt.setup(config_mod.resolve({ analysis_mode = "openFilesOnly" }))

    local tmppy = vim.fn.tempname() .. ".py"
    local f1 = io.open(tmppy, "w")
    if f1 then f1:write("x = 1\n"); f1:close() end
    vim.cmd("edit " .. vim.fn.fnameescape(tmppy))
    vim.wait(200)
    pcall(vim.cmd, "enew")
    vim.wait(200)
    os.remove(tmppy)
  end)

  -- ── health.lua ──────────────────────────────────────────────────────────

  it("health check runs", function()
    local health = require("basilisk.health")
    health.check()
  end)

  -- ── dap.lua ─────────────────────────────────────────────────────────────

  it("dap setup + stop_session", function()
    local dap_mod = require("basilisk.dap")
    local config_mod = require("basilisk.config")

    dap_mod.setup(config_mod.resolve({ debugger = { enabled = false } }))
    dap_mod.setup(config_mod.resolve({ debugger = { enabled = true } }))
    dap_mod.stop_session()
  end)
end)
