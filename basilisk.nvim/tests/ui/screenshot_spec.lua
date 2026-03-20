--- Screenshot regression tests using mini.test.
---
--- Captures terminal state for key UI elements and compares against
--- reference screenshots stored in tests/ui/screenshots/.
--- On first run, reference screenshots are auto-created.
---
--- Run:  nvim --headless -u tests/minimal_init.lua -l tests/ui/screenshot_spec.lua

local ok, MiniTest = pcall(require, "mini.test")
if not ok then
  -- mini.test not available — skip gracefully.
  print("mini.test not available — skipping screenshot tests")
  return
end

local helpers = require("tests.lsp.helpers")
local binary = helpers.find_binary()
if not binary then
  print("basilisk binary not found — skipping screenshot tests")
  return
end

local plugin_dir = vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":h:h:h")
local screenshot_dir = plugin_dir .. "/tests/ui/screenshots"

local new_set = MiniTest.new_set
local expect = MiniTest.expect

--- Create a child Neovim with basilisk configured.
local function make_child()
  local child = MiniTest.new_child_neovim()
  child.setup()

  -- Set up runtime path.
  child.lua("vim.opt.rtp:prepend(...)", { plugin_dir })
  child.lua("vim.opt.rtp:prepend('/tmp/plenary.nvim')")

  -- Minimal settings for consistent screenshots.
  child.lua([[
    vim.o.swapfile = false
    vim.o.number = true
    vim.o.signcolumn = "yes"
    vim.o.lines = 24
    vim.o.columns = 80
    vim.o.laststatus = 2
    vim.o.cmdheight = 1
    vim.cmd("filetype plugin indent on")
    vim.cmd("syntax enable")
  ]])

  return child
end

--- Create a temp dir with pyproject.toml for LSP root detection.
local function setup_project(child)
  local tmpdir = child.lua_get("vim.fn.tempname()")
  child.lua("vim.fn.mkdir(..., 'p')", { tmpdir })
  child.lua(
    [[local fh = io.open(...[1] .. "/pyproject.toml", "w"); fh:write('[project]\nname = "test"\nversion = "0.1.0"\n'); fh:close()]],
    { { tmpdir } }
  )
  return tmpdir
end

--- Start the basilisk LSP in the child.
local function start_lsp(child, tmpdir)
  child.lua(
    [[
      vim.lsp.config("basilisk", {
        cmd = { ...[1], "lsp" },
        filetypes = { "python" },
        root_markers = { "pyproject.toml" },
        settings = { basilisk = { analysisMode = "wholeModule" } },
      })
      vim.lsp.enable("basilisk")
    ]],
    { { binary } }
  )
end

--- Open a Python file in the child and wait for LSP.
local function open_and_wait(child, tmpdir, filename, content)
  local filepath = tmpdir .. "/" .. filename
  child.lua(
    [[
      local fh = io.open(...[1], "w"); fh:write(...[2]); fh:close()
      vim.cmd("edit " .. vim.fn.fnameescape(...[1]))
    ]],
    { { filepath, content } }
  )
  -- Wait for LSP to attach and produce diagnostics.
  child.lua([[
    vim.wait(8000, function()
      local clients = vim.lsp.get_clients({ bufnr = 0 })
      return #clients > 0
    end, 100)
    vim.wait(3000, function() return false end, 100)
  ]])
end

-- ── Test suite ───────────────────────────────────────────────────────────────

local T = new_set({
  hooks = {
    pre_case = function() end,
    post_case = function() end,
  },
})

-- 1. Diagnostics on untyped code

T["diagnostics_untyped"] = function()
  local child = make_child()
  local tmpdir = setup_project(child)
  start_lsp(child, tmpdir)

  open_and_wait(child, tmpdir, "bad.py", table.concat({
    "def greet(name):",
    "    return name",
    "",
    "def add(a, b):",
    "    return a + b",
    "",
    "x = greet('world')",
    "",
  }, "\n"))

  expect.reference_screenshot(child.get_screenshot(), nil, {
    directory = screenshot_dir,
  })

  child.stop()
end

-- 2. Clean code (no diagnostics)

T["diagnostics_clean"] = function()
  local child = make_child()
  local tmpdir = setup_project(child)
  start_lsp(child, tmpdir)

  open_and_wait(child, tmpdir, "good.py", table.concat({
    "def greet(name: str) -> str:",
    '    return "Hello " + name',
    "",
    "def add(a: int, b: int) -> int:",
    "    return a + b",
    "",
    'x: str = greet("world")',
    "",
  }, "\n"))

  expect.reference_screenshot(child.get_screenshot(), nil, {
    directory = screenshot_dir,
  })

  child.stop()
end

-- 3. :BasiliskInfo floating window

T["basilisk_info_float"] = function()
  local child = make_child()
  local tmpdir = setup_project(child)
  start_lsp(child, tmpdir)

  open_and_wait(child, tmpdir, "info.py", "x: int = 1\n")

  -- Register commands and open info float.
  child.lua(
    [[
      local basilisk = require("basilisk")
      basilisk.config = require("basilisk.config").resolve({ binary_path = ...[1] })
      require("basilisk.commands").register(basilisk.config)
      vim.cmd("BasiliskInfo")
      vim.wait(500)
    ]],
    { { binary } }
  )

  expect.reference_screenshot(child.get_screenshot(), nil, {
    directory = screenshot_dir,
  })

  child.stop()
end

-- 4. Test explorer panel

T["test_explorer_panel"] = function()
  local child = make_child()
  local tmpdir = setup_project(child)
  start_lsp(child, tmpdir)

  open_and_wait(child, tmpdir, "panel.py", "x: int = 1\n")

  -- Register commands and open test panel.
  child.lua(
    [[
      local basilisk = require("basilisk")
      basilisk.config = require("basilisk.config").resolve({ binary_path = ...[1] })
      require("basilisk.commands").register(basilisk.config)
      vim.cmd("BasiliskTestToggle")
      vim.wait(500)
    ]],
    { { binary } }
  )

  expect.reference_screenshot(child.get_screenshot(), nil, {
    directory = screenshot_dir,
  })

  child.stop()
end

-- 5. Diagnostic float

T["diagnostic_float"] = function()
  local child = make_child()
  local tmpdir = setup_project(child)
  start_lsp(child, tmpdir)

  open_and_wait(child, tmpdir, "diag_float.py", table.concat({
    "def greet(name):",
    "    return name",
    "",
  }, "\n"))

  -- Move cursor to error line and open diagnostic float.
  child.lua([[
    vim.api.nvim_win_set_cursor(0, { 1, 4 })
    vim.diagnostic.open_float()
    vim.wait(500)
  ]])

  expect.reference_screenshot(child.get_screenshot(), nil, {
    directory = screenshot_dir,
  })

  child.stop()
end

-- 6. Status line states

T["statusline_ready"] = function()
  local child = make_child()
  local tmpdir = setup_project(child)
  start_lsp(child, tmpdir)

  -- Configure statusline.
  child.lua([[
    local sl = require("basilisk.statusline")
    sl.set_state("ready")
    vim.o.statusline = "%{%v:lua.require('basilisk.statusline').get()%} %f"
  ]])

  open_and_wait(child, tmpdir, "status.py", "x: int = 1\n")

  -- Force a redraw.
  child.cmd("redraw!")

  expect.reference_screenshot(child.get_screenshot(), nil, {
    directory = screenshot_dir,
  })

  child.stop()
end

-- ── Run ──────────────────────────────────────────────────────────────────────

MiniTest.run({ collect = { find_files = function() return {} end } })
return T
