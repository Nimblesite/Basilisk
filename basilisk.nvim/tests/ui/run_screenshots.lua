--- Screenshot regression tests using mini.test.
---
--- Captures terminal state for key UI elements and compares against
--- reference screenshots stored in tests/ui/screenshots/.
--- On first run, reference screenshots are auto-created.
---
--- Run:  nvim --headless -u tests/minimal_init.lua -l tests/ui/run_screenshots.lua

local ok, MiniTest = pcall(require, "mini.test")
if not ok then
  print("SKIP: mini.test not available")
  vim.cmd("qa!")
  return
end

local helpers = require("tests.lsp.helpers")
local binary = helpers.find_binary()
if not binary then
  print("SKIP: basilisk binary not found")
  vim.cmd("qa!")
  return
end

local plugin_dir = vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":h:h:h")
local screenshot_dir = plugin_dir .. "/tests/ui/screenshots"

MiniTest.setup()

local new_set = MiniTest.new_set
local expect = MiniTest.expect

--- Create a child Neovim with basilisk configured.
local function make_child()
  local child = MiniTest.new_child_neovim()
  child.start()

  child.lua("vim.opt.rtp:prepend(...)", { plugin_dir })
  child.lua("vim.opt.rtp:prepend(...)", { "/tmp/plenary.nvim" })
  child.lua("vim.opt.rtp:prepend(...)", { "/tmp/mini.nvim" })

  child.lua([[
    vim.o.swapfile = false
    vim.o.number = true
    vim.o.signcolumn = "yes"
    vim.o.lines = 24
    vim.o.columns = 80
    vim.o.laststatus = 2
    vim.o.cmdheight = 1
    -- Stable statusline that won't contain random temp paths.
    vim.o.statusline = " %t %m%= %l,%c %P "
    vim.cmd("filetype plugin indent on")
    vim.cmd("syntax enable")
  ]])

  return child
end

local function setup_project(child)
  local tmpdir = child.lua_get("vim.fn.tempname()")
  child.lua("vim.fn.mkdir(..., 'p')", { tmpdir })
  child.lua([[
    local dir = select(1, ...)
    local fh = io.open(dir .. "/pyproject.toml", "w")
    fh:write('[project]\nname = "test"\nversion = "0.1.0"\n')
    fh:close()
  ]], { tmpdir })
  return tmpdir
end

local function start_lsp(child)
  child.lua([[
    local bin = select(1, ...)
    vim.lsp.config("basilisk", {
      cmd = { bin, "lsp" },
      filetypes = { "python" },
      root_markers = { "pyproject.toml" },
      settings = { basilisk = { analysisMode = "wholeModule" } },
    })
    vim.lsp.enable("basilisk")
  ]], { binary })
end

local function open_and_wait(child, tmpdir, filename, content)
  local filepath = tmpdir .. "/" .. filename
  child.lua([[
    local path, text = select(1, ...), select(2, ...)
    local fh = io.open(path, "w"); fh:write(text); fh:close()
    vim.cmd("edit " .. vim.fn.fnameescape(path))
  ]], { filepath, content })
  child.lua([[
    vim.wait(8000, function()
      return #vim.lsp.get_clients({ bufnr = 0 }) > 0
    end, 100)
    vim.wait(3000, function() return false end, 100)
  ]])
end

local function register_commands(child)
  child.lua([[
    local bin = select(1, ...)
    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ binary_path = bin })
    require("basilisk.commands").register(basilisk.config)
  ]], { binary })
end

-- ── Tests ────────────────────────────────────────────────────────────────────

local T = new_set()

T["diagnostics_untyped"] = function()
  local child = make_child()
  local tmpdir = setup_project(child)
  start_lsp(child)
  open_and_wait(child, tmpdir, "bad.py", "def greet(name):\n    return name\n\ndef add(a, b):\n    return a + b\n\nx = greet('world')\n")
  expect.reference_screenshot(child.get_screenshot(), nil, { directory = screenshot_dir })
  child.stop()
end

T["diagnostics_clean"] = function()
  local child = make_child()
  local tmpdir = setup_project(child)
  start_lsp(child)
  open_and_wait(child, tmpdir, "good.py", "def greet(name: str) -> str:\n    return 'Hello ' + name\n\ndef add(a: int, b: int) -> int:\n    return a + b\n\nx: str = greet('world')\n")
  expect.reference_screenshot(child.get_screenshot(), nil, { directory = screenshot_dir })
  child.stop()
end

T["basilisk_info_float"] = function()
  local child = make_child()
  local tmpdir = setup_project(child)
  start_lsp(child)
  open_and_wait(child, tmpdir, "info.py", "x: int = 1\n")
  register_commands(child)
  child.lua("vim.cmd('BasiliskInfo'); vim.wait(500)")
  -- ignore_text because the float contains random temp dir paths in Root field.
  expect.reference_screenshot(child.get_screenshot(), nil, { directory = screenshot_dir, ignore_text = true })
  child.stop()
end

T["test_explorer_panel"] = function()
  local child = make_child()
  local tmpdir = setup_project(child)
  start_lsp(child)
  open_and_wait(child, tmpdir, "panel.py", "x: int = 1\n")
  register_commands(child)
  child.lua("vim.cmd('BasiliskTestToggle'); vim.wait(500)")
  expect.reference_screenshot(child.get_screenshot(), nil, { directory = screenshot_dir })
  child.stop()
end

T["diagnostic_float"] = function()
  local child = make_child()
  local tmpdir = setup_project(child)
  start_lsp(child)
  open_and_wait(child, tmpdir, "diag_float.py", "def greet(name):\n    return name\n")
  child.lua([[
    vim.api.nvim_win_set_cursor(0, { 1, 4 })
    vim.diagnostic.open_float()
    vim.wait(500)
  ]])
  expect.reference_screenshot(child.get_screenshot(), nil, { directory = screenshot_dir })
  child.stop()
end

T["statusline_ready"] = function()
  local child = make_child()
  local tmpdir = setup_project(child)
  start_lsp(child)
  child.lua([[
    local sl = require("basilisk.statusline")
    sl.set_state("ready")
    vim.o.statusline = "%{%v:lua.require('basilisk.statusline').get()%} %f"
  ]])
  open_and_wait(child, tmpdir, "status.py", "x: int = 1\n")
  child.cmd("redraw!")
  -- ignore_text because the statusline contains the temp dir path.
  expect.reference_screenshot(child.get_screenshot(), nil, { directory = screenshot_dir, ignore_text = true })
  child.stop()
end

-- ── Execute ──────────────────────────────────────────────────────────────────

-- Guard against re-entry (run_file sources this file).
if _G._basilisk_screenshot_running then return T end
_G._basilisk_screenshot_running = true

local script_path = debug.getinfo(1, "S").source:sub(2)
MiniTest.run_file(script_path, {
  execute = { reporter = MiniTest.gen_reporter.stdout({}) },
})
