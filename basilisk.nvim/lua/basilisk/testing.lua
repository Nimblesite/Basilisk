--- Test explorer for Basilisk.
---
--- Discovers tests via pytest, displays a tree UI in a side panel,
--- and supports run/debug integration.
---
--- Implements [NVIM-TEST-EXPLORER] — Neovim tree UI, keymaps, and nvim-dap
--- integration for the test explorer (architecture in LSP-TEST-INTEGRATION-SPEC).

local log = require("basilisk.log")

local M = {}

--- Namespace for test diagnostics.
local ns = vim.api.nvim_create_namespace("basilisk-test")

--- Test tree data structure.
---@class BasiliskTestNode
---@field id string Fully qualified test ID.
---@field name string Display name.
---@field kind "file"|"class"|"function"
---@field file? string
---@field line? integer
---@field status "unknown"|"running"|"passed"|"failed"
---@field children BasiliskTestNode[]

--- The root test tree.
---@type BasiliskTestNode[]
local test_tree = {}

--- The test explorer buffer.
---@type integer?
local tree_buf = nil

--- The test explorer window.
---@type integer?
local tree_win = nil

--- Flat list of rendered node IDs (maps line number to test node).
---@type BasiliskTestNode[]
local rendered_nodes = {}

--- Status icons.
local STATUS_ICONS = {
  unknown = "○",
  running = "◌",
  passed = "●",
  failed = "✗",
}

--- Status highlight groups.
local STATUS_HL = {
  unknown = "Comment",
  running = "DiagnosticWarn",
  passed = "DiagnosticOk",
  failed = "DiagnosticError",
}

--- Parse pytest --collect-only output into a test tree.
---@param output string
---@return BasiliskTestNode[]
function M.parse_pytest_output(output)
  local tree = {}
  local file_nodes = {}
  local class_nodes = {}

  for line in output:gmatch("[^\n]+") do
    -- Skip empty lines and summary lines.
    if line:match("^%s*$") or line:match("^=") or line:match("^no tests") then
      goto continue
    end

    -- Parse test IDs: file.py::Class::test_name or file.py::test_name.
    local file, rest = line:match("^(.+%.py)::(.+)$")
    if not file then
      goto continue
    end

    -- Ensure file node exists.
    if not file_nodes[file] then
      file_nodes[file] = {
        id = file,
        name = vim.fn.fnamemodify(file, ":t"),
        kind = "file",
        file = file,
        status = "unknown",
        children = {},
      }
      tree[#tree + 1] = file_nodes[file]
    end
    local file_node = file_nodes[file]

    -- Split rest into class::test or just test.
    local class_name, test_name = rest:match("^(.+)::(.+)$")
    if class_name then
      local class_key = file .. "::" .. class_name
      if not class_nodes[class_key] then
        class_nodes[class_key] = {
          id = class_key,
          name = class_name,
          kind = "class",
          file = file,
          status = "unknown",
          children = {},
        }
        file_node.children[#file_node.children + 1] = class_nodes[class_key]
      end
      local class_node = class_nodes[class_key]
      class_node.children[#class_node.children + 1] = {
        id = line,
        name = test_name,
        kind = "function",
        file = file,
        status = "unknown",
        children = {},
      }
    else
      file_node.children[#file_node.children + 1] = {
        id = line,
        name = rest,
        kind = "function",
        file = file,
        status = "unknown",
        children = {},
      }
    end

    ::continue::
  end

  -- Update the module-level tree so refresh_display() picks it up.
  test_tree = tree

  return tree
end

--- Render the test tree into buffer lines.
---@param nodes BasiliskTestNode[]
---@param indent integer
---@param lines string[]
---@param node_map BasiliskTestNode[]
local function render_tree(nodes, indent, lines, node_map)
  local prefix = string.rep("  ", indent)
  for _, node in ipairs(nodes) do
    local icon = STATUS_ICONS[node.status] or "○"
    lines[#lines + 1] = prefix .. icon .. " " .. node.name
    node_map[#node_map + 1] = node
    if #node.children > 0 then
      render_tree(node.children, indent + 1, lines, node_map)
    end
  end
end

--- Refresh the tree buffer display.
function M.refresh_display()
  if not tree_buf or not vim.api.nvim_buf_is_valid(tree_buf) then
    return
  end

  local lines = {}
  rendered_nodes = {}
  render_tree(test_tree, 0, lines, rendered_nodes)

  if #lines == 0 then
    lines = { "  No tests discovered.", "  Run :BasiliskTestDiscover" }
  end

  vim.bo[tree_buf].modifiable = true
  vim.api.nvim_buf_set_lines(tree_buf, 0, -1, false, lines)
  vim.bo[tree_buf].modifiable = false
end

--- Get the test node at the current cursor line.
---@return BasiliskTestNode?
local function get_node_at_cursor()
  if not tree_win or not vim.api.nvim_win_is_valid(tree_win) then
    return nil
  end
  local row = vim.api.nvim_win_get_cursor(tree_win)[1]
  return rendered_nodes[row]
end

--- Discover tests using pytest.
---@param config BasiliskConfig
function M.discover(config)
  log.info("discovering tests...")

  local cmd = { config.test_explorer.pytest_path, "--collect-only", "-q" }
  for _, arg in ipairs(config.test_explorer.args) do
    cmd[#cmd + 1] = arg
  end

  vim.fn.jobstart(cmd, {
    stdout_buffered = true,
    on_stdout = function(_, data)
      if not data then
        return
      end
      local output = table.concat(data, "\n")
      vim.schedule(function()
        test_tree = M.parse_pytest_output(output)
        M.refresh_display()
        log.info("discovered %d test files", #test_tree)
      end)
    end,
    on_stderr = function(_, data)
      if data and data[1] ~= "" then
        log.debug("pytest stderr: %s", table.concat(data, "\n"))
      end
    end,
  })
end

--- Run a test (or all tests if no ID given).
---@param config BasiliskConfig
---@param test_id? string
function M.run(config, test_id)
  local cmd = { config.test_explorer.pytest_path, "-v", "--tb=short" }
  for _, arg in ipairs(config.test_explorer.args) do
    cmd[#cmd + 1] = arg
  end
  if test_id then
    cmd[#cmd + 1] = test_id
  end

  -- Mark as running.
  M.set_status(test_id, "running")
  M.refresh_display()

  vim.fn.jobstart(cmd, {
    stdout_buffered = true,
    on_stdout = function(_, data)
      if not data then
        return
      end
      vim.schedule(function()
        M.parse_test_results(table.concat(data, "\n"))
        M.refresh_display()
      end)
    end,
    on_exit = function(_, exit_code)
      vim.schedule(function()
        if exit_code == 0 then
          log.info("tests passed")
        else
          log.warn("tests failed (exit code %d)", exit_code)
        end
      end)
    end,
  })
end

--- Debug a test using nvim-dap.
---@param config BasiliskConfig
---@param test_id string
function M.debug(config, test_id)
  local dap_ok, dap = pcall(require, "dap")
  if not dap_ok then
    log.error("nvim-dap required for debugging tests")
    return
  end

  dap.run({
    type = "basilisk",
    request = "launch",
    name = "Debug: " .. test_id,
    module = "pytest",
    args = { "-xvs", test_id },
    justMyCode = true,
  })
end

--- Parse test results from pytest output and update the tree.
---@param output string
function M.parse_test_results(output)
  for line in output:gmatch("[^\n]+") do
    -- Match lines like: test_file.py::test_name PASSED/FAILED
    local test_id, result = line:match("^(.+%.py::.+)%s+(PASSED)")
    if not test_id then
      test_id, result = line:match("^(.+%.py::.+)%s+(FAILED)")
    end
    if test_id and result then
      local status = result == "PASSED" and "passed" or "failed"
      M.set_status(test_id, status)
    end
  end

  -- Set inline diagnostics for failures.
  M.update_diagnostics()
end

--- Set the status of a test node by ID.
---@param test_id? string
---@param status string
function M.set_status(test_id, status)
  if not test_id then
    return
  end
  local function walk(nodes)
    for _, node in ipairs(nodes) do
      if node.id == test_id then
        node.status = status
        return true
      end
      if walk(node.children) then
        return true
      end
    end
    return false
  end
  walk(test_tree)
end

--- Update inline diagnostics for failed tests.
function M.update_diagnostics()
  -- Clear previous diagnostics.
  vim.diagnostic.reset(ns)

  local diagnostics_by_buf = {}
  local function collect(nodes)
    for _, node in ipairs(nodes) do
      if node.status == "failed" and node.file and node.line then
        local bufnr = vim.fn.bufnr(node.file)
        if bufnr ~= -1 then
          if not diagnostics_by_buf[bufnr] then
            diagnostics_by_buf[bufnr] = {}
          end
          diagnostics_by_buf[bufnr][#diagnostics_by_buf[bufnr] + 1] = {
            lnum = (node.line or 1) - 1,
            col = 0,
            severity = vim.diagnostic.severity.ERROR,
            source = "basilisk-test",
            message = "Test failed: " .. node.name,
          }
        end
      end
      collect(node.children)
    end
  end
  collect(test_tree)

  for bufnr, diags in pairs(diagnostics_by_buf) do
    vim.diagnostic.set(ns, bufnr, diags)
  end
end

--- Create or show the test explorer panel.
---@param config BasiliskConfig
function M.open(config)
  if tree_win and vim.api.nvim_win_is_valid(tree_win) then
    vim.api.nvim_set_current_win(tree_win)
    return
  end

  -- Create buffer.
  tree_buf = vim.api.nvim_create_buf(false, true)
  vim.bo[tree_buf].filetype = "basilisk-tests"
  vim.bo[tree_buf].bufhidden = "wipe"
  vim.bo[tree_buf].swapfile = false

  -- Create split.
  local pos = config.test_explorer.position
  local width = config.test_explorer.width

  if pos == "bottom" then
    vim.cmd("botright split")
    vim.api.nvim_win_set_height(0, 15)
  elseif pos == "left" then
    vim.cmd("topleft vsplit")
    vim.api.nvim_win_set_width(0, width)
  else
    vim.cmd("botright vsplit")
    vim.api.nvim_win_set_width(0, width)
  end

  tree_win = vim.api.nvim_get_current_win()
  vim.api.nvim_win_set_buf(tree_win, tree_buf)
  vim.wo[tree_win].number = false
  vim.wo[tree_win].relativenumber = false
  vim.wo[tree_win].signcolumn = "no"
  vim.wo[tree_win].winfixwidth = true

  -- Set up keymaps.
  local buf = tree_buf
  vim.keymap.set("n", "<CR>", function()
    local node = get_node_at_cursor()
    if node and node.kind == "function" then
      M.run(config, node.id)
    end
  end, { buffer = buf, desc = "Run test" })

  vim.keymap.set("n", "d", function()
    local node = get_node_at_cursor()
    if node and node.kind == "function" then
      M.debug(config, node.id)
    end
  end, { buffer = buf, desc = "Debug test" })

  vim.keymap.set("n", "R", function()
    -- Re-run failed tests.
    local function collect_failed(nodes, ids)
      for _, node in ipairs(nodes) do
        if node.status == "failed" and node.kind == "function" then
          ids[#ids + 1] = node.id
        end
        collect_failed(node.children, ids)
      end
    end
    local failed = {}
    collect_failed(test_tree, failed)
    for _, id in ipairs(failed) do
      M.run(config, id)
    end
  end, { buffer = buf, desc = "Re-run failed tests" })

  vim.keymap.set("n", "q", function()
    M.close()
  end, { buffer = buf, desc = "Close test explorer" })

  M.refresh_display()
end

--- Close the test explorer panel.
function M.close()
  if tree_win and vim.api.nvim_win_is_valid(tree_win) then
    vim.api.nvim_win_close(tree_win, true)
  end
  tree_win = nil
  tree_buf = nil
end

--- Toggle the test explorer panel.
---@param config BasiliskConfig
function M.toggle(config)
  if tree_win and vim.api.nvim_win_is_valid(tree_win) then
    M.close()
  else
    M.open(config)
  end
end

--- Set up auto-discover on save.
---@param config BasiliskConfig
function M.setup_auto_discover(config)
  if not config.test_explorer.auto_discover_on_save then
    return
  end

  vim.api.nvim_create_autocmd("BufWritePost", {
    pattern = "*.py",
    group = vim.api.nvim_create_augroup("BasiliskTestAutoDiscover", { clear = true }),
    callback = function()
      if tree_buf and vim.api.nvim_buf_is_valid(tree_buf) then
        M.discover(config)
      end
    end,
  })
end

--- Parse coverage.xml and apply gutter highlights.
---@param coverage_path? string Path to coverage.xml. Defaults to "coverage.xml".
function M.apply_coverage(coverage_path)
  local path = coverage_path or "coverage.xml"
  local fh = io.open(path, "r")
  if not fh then
    log.debug("no coverage file found at %s", path)
    return
  end

  local content = fh:read("*a")
  fh:close()

  local cov_ns = vim.api.nvim_create_namespace("basilisk-coverage")

  -- Clear previous coverage marks.
  for _, buf in ipairs(vim.api.nvim_list_bufs()) do
    if vim.api.nvim_buf_is_loaded(buf) then
      vim.api.nvim_buf_clear_namespace(buf, cov_ns, 0, -1)
    end
  end

  -- Parse line coverage from XML (simplified parser for Cobertura format).
  -- Match <class filename="..."> and <line number="..." hits="..."/>
  local current_file = nil
  for line in content:gmatch("[^\n]+") do
    local filename = line:match('filename="([^"]+)"')
    if filename then
      current_file = filename
    end
    local line_num, hits = line:match('number="(%d+)"%s+hits="(%d+)"')
    if line_num and hits and current_file then
      local lnum = tonumber(line_num) - 1
      local hit_count = tonumber(hits)
      for _, buf in ipairs(vim.api.nvim_list_bufs()) do
        local name = vim.api.nvim_buf_get_name(buf)
        if name:find(current_file, 1, true) and vim.api.nvim_buf_is_loaded(buf) then
          local hl = hit_count > 0 and "DiagnosticOk" or "DiagnosticError"
          pcall(vim.api.nvim_buf_set_extmark, buf, cov_ns, lnum, 0, {
            sign_text = hit_count > 0 and "▎" or "▎",
            sign_hl_group = hl,
          })
        end
      end
    end
  end
end

return M
