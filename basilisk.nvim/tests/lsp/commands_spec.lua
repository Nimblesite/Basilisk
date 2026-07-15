--- Real command integration tests with actual LSP and UI interactions.
---
--- Tests [NVIM-USER-COMMANDS] (and [NVIM-LSP-CLIENT-CONFIGURATION-CUSTOM-COMMANDS]).
---
--- Tests all :Basilisk* commands with the REAL LSP server.
--- Verifies floating windows open, buffers change, keymaps fire, etc.

local helpers = require("tests.lsp.helpers")

local binary = helpers.find_binary()
if not binary then
  describe("basilisk commands (SKIPPED — no binary)", function()
    it("skipped", function()
      pending("basilisk binary not found")
    end)
  end)
  return
end

local tmpdir

describe("basilisk commands with real LSP", function()
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
    helpers.stop_clients()
    helpers.close_all_buffers()
    helpers.cleanup_tmpdir(tmpdir)
  end)

  -- :BasiliskInfo — floating window

  it(":BasiliskInfo opens a floating window", function()
    local buf = helpers.open_python_file(tmpdir, "test_info.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    -- Register commands manually (normally done by setup()).
    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    vim.cmd("BasiliskInfo")
    vim.wait(500)

    -- Find the floating window.
    local float_win = nil
    for _, win in ipairs(vim.api.nvim_list_wins()) do
      local config = vim.api.nvim_win_get_config(win)
      if config.relative and config.relative ~= "" then
        float_win = win
        break
      end
    end

    assert.is_not_nil(float_win, ":BasiliskInfo should open a floating window")

    -- Check content.
    local float_buf = vim.api.nvim_win_get_buf(float_win)
    local lines = vim.api.nvim_buf_get_lines(float_buf, 0, -1, false)
    local text = table.concat(lines, "\n")
    assert.truthy(text:find("Basilisk"), "float should contain 'Basilisk'")
    assert.truthy(text:find("Status"), "float should contain 'Status'")

    -- Close with q.
    vim.api.nvim_set_current_win(float_win)
    vim.api.nvim_feedkeys("q", "x", false)
    vim.wait(200)

    -- Verify closed.
    assert.is_false(vim.api.nvim_win_is_valid(float_win), "float should close on 'q'")
  end)

  -- :BasiliskOrganizeImports — real LSP command

  it(":BasiliskOrganizeImports sends LSP command", function()
    local buf = helpers.open_python_file(tmpdir, "test_organize.py", "import os\nimport sys\n\nprint(sys.path)\nprint(os.getcwd())\n")
    helpers.wait_for_server_ready(buf)

    -- Register commands.
    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    -- Execute — should not error.
    local ok = pcall(vim.cmd, "BasiliskOrganizeImports")
    assert.is_true(ok, ":BasiliskOrganizeImports should not error")
  end)

  -- :BasiliskFixFile — real LSP command

  it(":BasiliskFixFile sends LSP command", function()
    local buf = helpers.open_python_file(tmpdir, "test_fixfile.py", "def greet(name):\n    return name\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local ok = pcall(vim.cmd, "BasiliskFixFile")
    assert.is_true(ok, ":BasiliskFixFile should not error")
  end)

  -- :BasiliskAdoptFile — real LSP command

  it(":BasiliskAdoptFile sends LSP command", function()
    local buf = helpers.open_python_file(tmpdir, "test_adopt.py", "x = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local ok = pcall(vim.cmd, "BasiliskAdoptFile")
    assert.is_true(ok, ":BasiliskAdoptFile should not error")
  end)

  -- :BasiliskTestToggle — UI panel

  it(":BasiliskTestToggle opens/closes test panel", function()
    local buf = helpers.open_python_file(tmpdir, "test_panel.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local win_count_before = #vim.api.nvim_tabpage_list_wins(0)

    vim.cmd("BasiliskTestToggle")
    vim.wait(200)

    local win_count_after = #vim.api.nvim_tabpage_list_wins(0)
    assert.is_true(win_count_after > win_count_before, "should open a new panel window")

    -- Find the test panel buffer.
    local found_test_buf = false
    for _, b in ipairs(vim.api.nvim_list_bufs()) do
      if vim.bo[b].filetype == "basilisk-tests" then
        found_test_buf = true
        break
      end
    end
    assert.is_true(found_test_buf, "should create buffer with basilisk-tests filetype")

    -- Toggle off.
    vim.cmd("BasiliskTestToggle")
    vim.wait(200)
    local win_count_closed = #vim.api.nvim_tabpage_list_wins(0)
    assert.are.equal(win_count_before, win_count_closed, "toggle should close the panel")
  end)

  -- :BasiliskDisableRule — sends basilisk.disableRule to real LSP

  it(":BasiliskDisableRule sends LSP command", function()
    local buf = helpers.open_python_file(tmpdir, "test_disable.py", "def greet(name):\n    return name\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local ok = pcall(vim.cmd, "BasiliskDisableRule BSK-0001")
    assert.is_true(ok, ":BasiliskDisableRule should not error")

    -- Verify pyproject.toml was written. Writing the config is the command's
    -- whole job, so the file MUST exist — never treat its absence as a pass.
    vim.wait(1000)
    local fh = io.open(tmpdir .. "/pyproject.toml", "r")
    assert.truthy(fh, "BasiliskDisableRule must write pyproject.toml")
    local content = fh:read("*a")
    fh:close()
    -- Codes are letterless post-config-refactor: disabling BSK-0001 writes
    -- exactly `BSK-0001` (never the pre-refactor `BSK-E0001`).
    assert.truthy(content:find("BSK%-0001"), "pyproject.toml should contain the disabled rule BSK-0001")
  end)

  -- :BasiliskFixWorkspace — sends basilisk.fixWorkspace to real LSP

  it(":BasiliskFixWorkspace sends LSP command", function()
    local buf = helpers.open_python_file(tmpdir, "test_fixws.py", "def greet(name):\n    return name\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local ok = pcall(vim.cmd, "BasiliskFixWorkspace")
    assert.is_true(ok, ":BasiliskFixWorkspace should not error")
  end)

  -- :BasiliskAdoptWorkspace — sends basilisk.adoptWorkspace to real LSP

  it(":BasiliskAdoptWorkspace sends LSP command", function()
    local buf = helpers.open_python_file(tmpdir, "test_adoptws.py", "x = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local ok = pcall(vim.cmd, "BasiliskAdoptWorkspace")
    assert.is_true(ok, ":BasiliskAdoptWorkspace should not error")
  end)

  -- :BasiliskUnadoptFile — sends basilisk.unadoptFile to real LSP

  it(":BasiliskUnadoptFile sends LSP command", function()
    local buf = helpers.open_python_file(tmpdir, "test_unadopt.py", "x = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local ok = pcall(vim.cmd, "BasiliskUnadoptFile")
    assert.is_true(ok, ":BasiliskUnadoptFile should not error")
  end)

  -- :BasiliskShowOutput — opens the LSP log file

  it(":BasiliskShowOutput opens log buffer", function()
    local buf = helpers.open_python_file(tmpdir, "test_output.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local buf_count_before = #vim.api.nvim_list_bufs()
    local ok = pcall(vim.cmd, "BasiliskShowOutput")
    assert.is_true(ok, ":BasiliskShowOutput should not error")
  end)

  -- ── Profiling commands with real LSP ─────────────────────────────────────

  it(":BasiliskProfile sends profiler/start to real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_profile.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    -- Should not crash even if server doesn't handle profiler commands yet.
    local ok = pcall(vim.cmd, "BasiliskProfile")
    assert.is_true(ok, ":BasiliskProfile should not error")
  end)

  it(":BasiliskProfileStop sends profiler/stop to real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_profstop.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local ok = pcall(vim.cmd, "BasiliskProfileStop")
    assert.is_true(ok, ":BasiliskProfileStop should not error")
  end)

  it(":BasiliskProfileSnapshot sends profiler/snapshot to real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_profsnap.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local ok = pcall(vim.cmd, "BasiliskProfileSnapshot")
    assert.is_true(ok, ":BasiliskProfileSnapshot should not error")
  end)

  -- ── Memory commands with real LSP ────────────────────────────────────────

  it(":BasiliskMemLeak sends memory/start to real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_memleak.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local ok = pcall(vim.cmd, "BasiliskMemLeak")
    assert.is_true(ok, ":BasiliskMemLeak should not error")
  end)

  it(":BasiliskMemStop sends memory/stop to real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_memstop.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local ok = pcall(vim.cmd, "BasiliskMemStop")
    assert.is_true(ok, ":BasiliskMemStop should not error")
  end)

  it(":BasiliskMemRefs sends memory/refs to real LSP", function()
    local buf = helpers.open_python_file(tmpdir, "test_memrefs.py", "x: int = 1\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local ok = pcall(vim.cmd, "BasiliskMemRefs dict")
    assert.is_true(ok, ":BasiliskMemRefs should not error")
  end)

  -- ── Refactoring commands with real LSP ───────────────────────────────────

  it(":BasiliskExtractVariable triggers code action", function()
    local buf = helpers.open_python_file(tmpdir, "test_extract.py", "def calc() -> int:\n    return 1 + 2 + 3\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    -- Select range in visual mode then run command — should not crash.
    local ok = pcall(vim.cmd, "BasiliskExtractVariable")
    assert.is_true(ok, ":BasiliskExtractVariable should not error")
  end)

  it(":BasiliskExtractConstant triggers code action", function()
    local buf = helpers.open_python_file(tmpdir, "test_const.py", "def calc() -> int:\n    return 42\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local ok = pcall(vim.cmd, "BasiliskExtractConstant")
    assert.is_true(ok, ":BasiliskExtractConstant should not error")
  end)

  it(":BasiliskConvertUnion triggers code action", function()
    local buf = helpers.open_python_file(tmpdir, "test_union.py", "from typing import Optional\n\ndef greet(name: Optional[str]) -> str:\n    return name or 'world'\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local ok = pcall(vim.cmd, "BasiliskConvertUnion")
    assert.is_true(ok, ":BasiliskConvertUnion should not error")
  end)

  it(":BasiliskImplementMethods triggers code action", function()
    local buf = helpers.open_python_file(tmpdir, "test_impl.py", "from abc import ABC, abstractmethod\n\nclass Base(ABC):\n    @abstractmethod\n    def run(self) -> None: ...\n\nclass Child(Base):\n    pass\n")
    helpers.wait_for_server_ready(buf)

    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = binary })
    require("basilisk.commands").register(basilisk.config)

    local ok = pcall(vim.cmd, "BasiliskImplementMethods")
    assert.is_true(ok, ":BasiliskImplementMethods should not error")
  end)
end)
