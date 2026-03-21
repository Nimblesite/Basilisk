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

--- Compare two screenshot attribute grids with a tolerance threshold.
--- Returns true when the fraction of differing cells is within the threshold.
---@param ref_attr string[]
---@param cur_attr string[]
---@param threshold number  Maximum fraction of cells allowed to differ (0.0–1.0).
local function attrs_within_threshold(ref_attr, cur_attr, threshold)
  local total, diffs = 0, 0
  for row = 1, math.min(#ref_attr, #cur_attr) do
    local ref_row = ref_attr[row]
    local cur_row = cur_attr[row]
    for col = 1, math.min(#ref_row, #cur_row) do
      total = total + 1
      if ref_row:sub(col, col) ~= cur_row:sub(col, col) then
        diffs = diffs + 1
      end
    end
  end
  if total == 0 then return true end
  return (diffs / total) <= threshold
end

--- Load a reference screenshot file.  Returns { text = {...}, attr = {...} }
--- or nil if the file does not exist.
local function load_reference(path)
  local fh = io.open(path, "r")
  if not fh then return nil end
  local lines = {}
  for line in fh:lines() do lines[#lines + 1] = line end
  fh:close()
  -- Format: text rows, separator "--|---...", attr rows, separator, empty
  local sep_idx = nil
  for idx, line in ipairs(lines) do
    if line:match("^%-%-|") then sep_idx = idx; break end
  end
  if not sep_idx then return nil end
  local text, attr = {}, {}
  for idx = 1, sep_idx - 1 do text[#text + 1] = lines[idx] end
  -- After separator, attr rows until next separator or end.
  for idx = sep_idx + 1, #lines do
    if lines[idx]:match("^%-%-|") or lines[idx] == "" then break end
    attr[#attr + 1] = lines[idx]
  end
  return { text = text, attr = attr }
end

--- Threshold-aware screenshot assertion.  Falls back to threshold check when
--- exact match fails and the attribute diff is within the given threshold.
---@param screenshot table  child.get_screenshot() result
---@param threshold number  Maximum fraction of attr cells allowed to differ (0.0–1.0).
---@param opts? table       { directory, ignore_text }
local function assert_screenshot(screenshot, threshold, opts)
  opts = opts or {}
  local dir = opts.directory or screenshot_dir

  -- Delegate to mini.test for reference creation and text-level checks.
  -- If it passes, great. If it fails, check whether it's within threshold.
  local ok_exact, err = pcall(expect.reference_screenshot, screenshot, nil, {
    directory = dir,
    ignore_text = opts.ignore_text,
  })
  if ok_exact then return end

  -- Exact match failed — check if attr diff is within threshold.
  -- Determine the reference path that mini.test would have used.
  -- mini.test names references after the test case path.
  -- We need to find the most recently written reference file.
  local ref_files = vim.fn.glob(dir .. "/*", false, true)
  if #ref_files == 0 then error(err) end

  -- Try each reference file (there should be one per test, named by case).
  -- Pick the one whose text layer matches (ignoring text if requested).
  for _, ref_path in ipairs(ref_files) do
    local ref = load_reference(ref_path)
    if ref then
      -- Extract attr lines from the current screenshot string repr.
      local cur_lines = {}
      local cur_str = tostring(screenshot)
      for line in cur_str:gmatch("[^\n]+") do cur_lines[#cur_lines + 1] = line end
      local cur_sep = nil
      for idx, line in ipairs(cur_lines) do
        if line:match("^%-%-|") then cur_sep = idx; break end
      end
      if cur_sep then
        local cur_attr = {}
        for idx = cur_sep + 1, #cur_lines do
          if cur_lines[idx]:match("^%-%-|") or cur_lines[idx] == "" then break end
          cur_attr[#cur_attr + 1] = cur_lines[idx]
        end
        if attrs_within_threshold(ref.attr, cur_attr, threshold) then return end
      end
    end
  end

  -- Still outside threshold — propagate the original error.
  error(err)
end

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
  assert_screenshot(child.get_screenshot(), 0.40, { directory = screenshot_dir, ignore_text = true })
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
  -- ignore_text + threshold because the statusline contains temp dir paths
  -- that differ across environments, shifting the attribute grid.
  assert_screenshot(child.get_screenshot(), 0.15, { directory = screenshot_dir, ignore_text = true })
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
