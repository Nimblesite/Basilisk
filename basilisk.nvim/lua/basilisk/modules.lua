--- Module Explorer panel for Basilisk.
---
--- Renders the workspace module tree in a split buffer. Data is fetched
--- from the LSP server via `basilisk.workspaceModules`.

local ui = require("basilisk.ui")
local log = require("basilisk.log")

local M = {}

--- State for the module explorer buffer.
---@type integer?
local modules_buf = nil
---@type integer?
local modules_win = nil

--- Fold state tracking: set of module names that are collapsed.
---@type table<string, boolean>
local collapsed = {}

--- Cached module data from last fetch.
---@type table[]?
local cached_modules = nil

--- Fetch module tree from the LSP server.
---@param callback fun(modules: table[])
local function fetch_modules(callback)
  local client = ui.get_client()
  if not client then
    log.warn("no active LSP client for module explorer")
    callback({})
    return
  end
  client:request("workspace/executeCommand", {
    command = "basilisk.workspaceModules",
    arguments = { {} },
  }, function(err, result)
    if err then
      log.error("workspaceModules failed: %s", err.message or tostring(err))
      callback({})
      return
    end
    local modules = (result and result.modules) or {}
    cached_modules = modules
    callback(modules)
  end, 0)
end

--- Render the module tree into buffer lines.
---@param modules table[]
---@return string[]
---@return table[] highlights  { line, col_start, col_end, hl_group }
local function render_tree(modules)
  local lines = {}
  local highlights = {}

  for _, mod in ipairs(modules) do
    local is_collapsed = collapsed[mod.name]
    local icon = is_collapsed and "▸" or "▾"
    local kind_label = mod.kind == "package" and "[pkg]" or "[mod]"
    lines[#lines + 1] = string.format("%s %s %s", icon, mod.name, kind_label)
    highlights[#highlights + 1] = {
      line = #lines - 1,
      col_start = 0,
      col_end = #icon,
      hl_group = "Directory",
    }

    if not is_collapsed and mod.symbols then
      for _, sym in ipairs(mod.symbols) do
        local sym_icon = ({
          class = "●",
          ["function"] = "ƒ",
          variable = "◆",
          constant = "◇",
          typeAlias = "τ",
        })[sym.kind] or "·"

        local annotation = sym.annotated and "" or " [untyped]"
        local private = (sym.name:sub(1, 1) == "_" and sym.name:sub(1, 2) ~= "__") and " (private)" or ""
        lines[#lines + 1] = string.format("  %s %s%s%s", sym_icon, sym.name, annotation, private)

        -- Highlight unannotated symbols.
        if not sym.annotated then
          highlights[#highlights + 1] = {
            line = #lines - 1,
            col_start = 0,
            col_end = #lines[#lines],
            hl_group = "DiagnosticWarn",
          }
        end

        -- Render class children.
        if sym.children then
          for _, child in ipairs(sym.children) do
            local child_ann = child.annotated and "" or " [untyped]"
            lines[#lines + 1] = string.format("    · %s%s", child.name, child_ann)
            if not child.annotated then
              highlights[#highlights + 1] = {
                line = #lines - 1,
                col_start = 0,
                col_end = #lines[#lines],
                hl_group = "DiagnosticWarn",
              }
            end
          end
        end
      end
    end
  end

  if #lines == 0 then
    lines = { "  (no modules found)" }
  end

  return lines, highlights
end

--- Apply highlight groups to the buffer.
---@param buf integer
---@param highlights table[]
local function apply_highlights(buf, highlights)
  local ns = vim.api.nvim_create_namespace("basilisk_modules")
  vim.api.nvim_buf_clear_namespace(buf, ns, 0, -1)
  for _, hl in ipairs(highlights) do
    vim.api.nvim_buf_add_highlight(buf, ns, hl.hl_group, hl.line, hl.col_start, hl.col_end)
  end
end

--- Refresh the modules buffer content.
local function refresh_buffer()
  if not modules_buf or not vim.api.nvim_buf_is_valid(modules_buf) then
    return
  end
  local modules = cached_modules or {}
  local lines, highlights = render_tree(modules)
  vim.bo[modules_buf].modifiable = true
  vim.api.nvim_buf_set_lines(modules_buf, 0, -1, false, lines)
  vim.bo[modules_buf].modifiable = false
  apply_highlights(modules_buf, highlights)
end

--- Set up keybindings for the modules buffer.
---@param buf integer
local function setup_keybindings(buf)
  local opts = { buffer = buf, nowait = true }

  -- <CR> - open file at symbol.
  vim.keymap.set("n", "<CR>", function()
    local line = vim.api.nvim_win_get_cursor(0)[1]
    local text = vim.api.nvim_buf_get_lines(buf, line - 1, line, false)[1] or ""
    -- Find the module for this line by scanning cached_modules.
    if cached_modules then
      local current_line = 0
      for _, mod in ipairs(cached_modules) do
        current_line = current_line + 1
        if current_line == line then
          vim.cmd("wincmd p")
          vim.cmd("edit " .. vim.fn.fnameescape(mod.path))
          return
        end
        if not collapsed[mod.name] and mod.symbols then
          for _, sym in ipairs(mod.symbols) do
            current_line = current_line + 1
            if current_line == line then
              vim.cmd("wincmd p")
              vim.cmd("edit " .. vim.fn.fnameescape(mod.path))
              vim.api.nvim_win_set_cursor(0, { sym.line + 1, 0 })
              return
            end
            if sym.children then
              current_line = current_line + #sym.children
            end
          end
        end
      end
    end
  end, opts)

  -- o - toggle fold.
  vim.keymap.set("n", "o", function()
    local line = vim.api.nvim_win_get_cursor(0)[1]
    if cached_modules then
      local current_line = 0
      for _, mod in ipairs(cached_modules) do
        current_line = current_line + 1
        if current_line == line then
          collapsed[mod.name] = not collapsed[mod.name]
          refresh_buffer()
          return
        end
        if not collapsed[mod.name] and mod.symbols then
          for _, sym in ipairs(mod.symbols) do
            current_line = current_line + 1
            if sym.children then
              current_line = current_line + #sym.children
            end
          end
        end
      end
    end
  end, opts)

  -- r - refresh.
  vim.keymap.set("n", "r", function()
    fetch_modules(function()
      vim.schedule(refresh_buffer)
    end)
  end, opts)

  -- y - copy import path.
  vim.keymap.set("n", "y", function()
    local line = vim.api.nvim_win_get_cursor(0)[1]
    if cached_modules then
      local current_line = 0
      for _, mod in ipairs(cached_modules) do
        current_line = current_line + 1
        if current_line == line then
          vim.fn.setreg("+", "import " .. mod.name)
          log.info("copied: import %s", mod.name)
          return
        end
        if not collapsed[mod.name] and mod.symbols then
          for _, sym in ipairs(mod.symbols) do
            current_line = current_line + 1
            if current_line == line then
              local import_path = string.format("from %s import %s", mod.name, sym.name)
              vim.fn.setreg("+", import_path)
              log.info("copied: %s", import_path)
              return
            end
            if sym.children then
              current_line = current_line + #sym.children
            end
          end
        end
      end
    end
  end, opts)

  -- q - close.
  vim.keymap.set("n", "q", function()
    M.close()
  end, opts)
end

--- Open the module explorer in a vertical split.
function M.open()
  if modules_win and vim.api.nvim_win_is_valid(modules_win) then
    vim.api.nvim_set_current_win(modules_win)
    return
  end

  modules_buf = vim.api.nvim_create_buf(false, true)
  vim.bo[modules_buf].bufhidden = "wipe"
  vim.bo[modules_buf].filetype = "basilisk-modules"
  vim.bo[modules_buf].modifiable = false

  vim.cmd("topleft 40vsplit")
  modules_win = vim.api.nvim_get_current_win()
  vim.api.nvim_win_set_buf(modules_win, modules_buf)
  vim.wo[modules_win].number = false
  vim.wo[modules_win].relativenumber = false
  vim.wo[modules_win].signcolumn = "no"
  vim.wo[modules_win].wrap = false
  vim.wo[modules_win].winfixwidth = true

  setup_keybindings(modules_buf)

  fetch_modules(function()
    vim.schedule(refresh_buffer)
  end)
end

--- Close the module explorer.
function M.close()
  if modules_win and vim.api.nvim_win_is_valid(modules_win) then
    vim.api.nvim_win_close(modules_win, true)
  end
  modules_win = nil
  modules_buf = nil
end

--- Toggle the module explorer.
function M.toggle()
  if modules_win and vim.api.nvim_win_is_valid(modules_win) then
    M.close()
  else
    M.open()
  end
end

--- Refresh the module explorer (called from notification handler).
function M.refresh()
  if modules_buf and vim.api.nvim_buf_is_valid(modules_buf) then
    fetch_modules(function()
      vim.schedule(refresh_buffer)
    end)
  end
end

return M
