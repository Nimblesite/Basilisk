--- Type Health panel for Basilisk.
---
--- Renders per-module type coverage statistics in a split buffer with
--- colored highlights. Data is fetched from the LSP server via
--- `basilisk.typeHealth`.

local ui = require("basilisk.ui")
local log = require("basilisk.log")

local M = {}

---@type integer?
local health_buf = nil
---@type integer?
local health_win = nil

--- Fetch type health from the LSP server.
---@param callback fun(data: table)
local function fetch_health(callback)
  local client = ui.get_client()
  if not client then
    log.warn("no active LSP client for type health")
    callback({})
    return
  end
  client:request("workspace/executeCommand", {
    command = "basilisk.typeHealth",
    arguments = { {} },
  }, function(err, result)
    if err then
      log.error("typeHealth failed: %s", err.message or tostring(err))
      callback({})
      return
    end
    callback(result or {})
  end, 0)
end

--- Build a text progress bar.
---@param percent number
---@param width? number
---@return string
local function progress_bar(percent, width)
  width = width or 20
  local filled = math.floor(percent / 100 * width + 0.5)
  return string.rep("█", filled) .. string.rep("░", width - filled)
end

--- Render type health data into lines and highlights.
---@param data table
---@return string[]
---@return table[]
local function render_health(data)
  local lines = {}
  local highlights = {}
  local ws = data.workspace or {}

  -- Header.
  lines[#lines + 1] = "Type Health — Workspace Summary"
  lines[#lines + 1] = ""
  lines[#lines + 1] = string.format(
    "  Coverage:  %s %d%%",
    progress_bar(ws.coveragePercent or 100),
    ws.coveragePercent or 100
  )
  lines[#lines + 1] = string.format(
    "  Symbols:   %d / %d annotated",
    ws.annotatedSymbols or 0,
    ws.totalSymbols or 0
  )
  lines[#lines + 1] = string.format("  Errors:    %d", ws.errors or 0)
  lines[#lines + 1] = string.format("  Warnings:  %d", ws.warnings or 0)
  lines[#lines + 1] = string.format(
    "  Files:     %d (%d adopted)",
    ws.totalFiles or 0,
    ws.adoptedFiles or 0
  )
  lines[#lines + 1] = ""

  -- Highlight the coverage line.
  local cov = ws.coveragePercent or 100
  local cov_hl = cov >= 90 and "DiagnosticOk" or (cov >= 50 and "DiagnosticWarn" or "DiagnosticError")
  highlights[#highlights + 1] = { line = 2, col_start = 0, col_end = #lines[3], hl_group = cov_hl }

  -- Per-module table.
  lines[#lines + 1] = "Per-Module Breakdown (sorted by coverage)"
  lines[#lines + 1] = string.rep("─", 60)

  local modules = data.modules or {}
  for _, mod in ipairs(modules) do
    local badge = mod.adopted and " [adopted]" or ""
    local issues = {}
    if mod.errors > 0 then issues[#issues + 1] = mod.errors .. "E" end
    if mod.warnings > 0 then issues[#issues + 1] = mod.warnings .. "W" end
    local issue_str = #issues > 0 and (" — " .. table.concat(issues, " ")) or ""

    lines[#lines + 1] = string.format(
      "  %s %3d%% %s%s%s",
      progress_bar(mod.coveragePercent, 10),
      mod.coveragePercent,
      mod.name,
      issue_str,
      badge
    )

    -- Color by coverage level.
    local mod_hl = mod.coveragePercent >= 90 and "DiagnosticOk"
      or (mod.coveragePercent >= 50 and "DiagnosticWarn" or "DiagnosticError")
    highlights[#highlights + 1] = {
      line = #lines - 1,
      col_start = 0,
      col_end = #lines[#lines],
      hl_group = mod_hl,
    }
  end

  if #modules == 0 then
    lines[#lines + 1] = "  (no modules analysed)"
  end

  return lines, highlights
end

--- Apply highlights to the buffer.
---@param buf integer
---@param highlights table[]
local function apply_highlights(buf, highlights)
  local ns = vim.api.nvim_create_namespace("basilisk_type_health")
  vim.api.nvim_buf_clear_namespace(buf, ns, 0, -1)
  for _, hl in ipairs(highlights) do
    vim.api.nvim_buf_add_highlight(buf, ns, hl.hl_group, hl.line, hl.col_start, hl.col_end)
  end
end

--- Open the type health panel in a split buffer.
function M.open()
  if health_win and vim.api.nvim_win_is_valid(health_win) then
    vim.api.nvim_set_current_win(health_win)
    return
  end

  health_buf = vim.api.nvim_create_buf(false, true)
  vim.bo[health_buf].bufhidden = "wipe"
  vim.bo[health_buf].filetype = "basilisk-health"
  vim.bo[health_buf].modifiable = false

  vim.cmd("botright 15split")
  health_win = vim.api.nvim_get_current_win()
  vim.api.nvim_win_set_buf(health_win, health_buf)
  vim.wo[health_win].number = false
  vim.wo[health_win].relativenumber = false
  vim.wo[health_win].signcolumn = "no"
  vim.wo[health_win].wrap = false

  -- Keybindings.
  local opts = { buffer = health_buf, nowait = true }
  vim.keymap.set("n", "q", function() M.close() end, opts)
  vim.keymap.set("n", "r", function() M.refresh() end, opts)
  vim.keymap.set("n", "<CR>", function()
    -- Open module file at cursor.
    local line = vim.api.nvim_win_get_cursor(0)[1]
    local text = vim.api.nvim_buf_get_lines(health_buf, line - 1, line, false)[1] or ""
    -- Extract module name from the line (after percentage).
    local name = text:match("%d+%% (.+)")
    if name then
      name = name:gsub(" %— .*", ""):gsub(" %[adopted%]", ""):gsub("^%s+", "")
    end
    if not name then return end
    -- Look up path from LSP.
    local client = ui.get_client()
    if client then
      client:request("workspace/executeCommand", {
        command = "basilisk.workspaceModules",
        arguments = { { scope = name } },
      }, function(err, result)
        if err or not result then return end
        for _, mod in ipairs(result.modules or {}) do
          if mod.name == name then
            vim.schedule(function()
              vim.cmd("wincmd p")
              vim.cmd("edit " .. vim.fn.fnameescape(mod.path))
            end)
            return
          end
        end
      end, 0)
    end
  end, opts)

  M.refresh()
end

--- Close the type health panel.
function M.close()
  if health_win and vim.api.nvim_win_is_valid(health_win) then
    vim.api.nvim_win_close(health_win, true)
  end
  health_win = nil
  health_buf = nil
end

--- Refresh the type health panel.
function M.refresh()
  if not health_buf or not vim.api.nvim_buf_is_valid(health_buf) then
    return
  end
  fetch_health(function(data)
    vim.schedule(function()
      if not health_buf or not vim.api.nvim_buf_is_valid(health_buf) then
        return
      end
      local lines, highlights = render_health(data)
      vim.bo[health_buf].modifiable = true
      vim.api.nvim_buf_set_lines(health_buf, 0, -1, false, lines)
      vim.bo[health_buf].modifiable = false
      apply_highlights(health_buf, highlights)
    end)
  end)
end

--- Toggle the type health panel.
function M.toggle()
  if health_win and vim.api.nvim_win_is_valid(health_win) then
    M.close()
  else
    M.open()
  end
end

return M
