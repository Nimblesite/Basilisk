--- Code lens activation tests for ftplugin/python.lua.
---
--- Regression coverage for the deprecated `vim.lsp.codelens.refresh()` call,
--- which spams a warning on Neovim 0.12 and stops working on 0.13. The
--- ftplugin must prefer `vim.lsp.codelens.enable()` when the running Neovim
--- exposes it, and only fall back to `refresh()` on older versions.
---
--- See https://github.com/Nimblesite/Basilisk/issues/66.
---
--- These tests mock the LSP client and the codelens API, so they run without
--- the real basilisk binary and on any Neovim version.

local function make_python_buffer()
  -- Open a Python buffer so ftplugin/python.lua is sourced and registers its
  -- buffer-local LspAttach handler. The buffer must be current when the
  -- filetype is set, because the ftplugin scopes its autocmd to buffer 0.
  local buf = vim.api.nvim_create_buf(true, false)
  vim.api.nvim_buf_set_name(buf, vim.fn.tempname() .. ".py")
  vim.api.nvim_set_current_buf(buf)
  vim.bo[buf].filetype = "python"
  return buf
end

describe("basilisk ftplugin code lens activation", function()
  local orig_enable
  local orig_refresh
  local orig_get_client_by_id

  before_each(function()
    -- The ftplugin guard requires basilisk.config to be present.
    local basilisk = require("basilisk")
    basilisk.config = require("basilisk.config").resolve({ keymaps = { enabled = false } })

    orig_enable = vim.lsp.codelens.enable
    orig_refresh = vim.lsp.codelens.refresh
    orig_get_client_by_id = vim.lsp.get_client_by_id

    -- Mock a basilisk client that advertises code lens support.
    vim.lsp.get_client_by_id = function(_)
      return {
        name = "basilisk",
        supports_method = function(_, method)
          return method == "textDocument/codeLens"
        end,
      }
    end
  end)

  after_each(function()
    vim.lsp.codelens.enable = orig_enable
    vim.lsp.codelens.refresh = orig_refresh
    vim.lsp.get_client_by_id = orig_get_client_by_id

    for _, buf in ipairs(vim.api.nvim_list_bufs()) do
      if vim.api.nvim_buf_is_valid(buf) then
        pcall(vim.api.nvim_buf_delete, buf, { force = true })
      end
    end
  end)

  it("prefers vim.lsp.codelens.enable over deprecated refresh on Neovim 0.12+", function()
    -- Simulate an nvim 0.12+ runtime where enable() exists.
    local enable_calls = {}
    local refresh_calls = {}
    vim.lsp.codelens.enable = function(on, opts)
      table.insert(enable_calls, { on = on, opts = opts })
    end
    vim.lsp.codelens.refresh = function(opts)
      table.insert(refresh_calls, { opts = opts })
    end

    local buf = make_python_buffer()

    vim.api.nvim_exec_autocmds("LspAttach", {
      buffer = buf,
      data = { client_id = 1 },
    })

    assert.are.equal(1, #enable_calls, "ftplugin should call vim.lsp.codelens.enable exactly once on attach")
    assert.is_true(enable_calls[1].on, "enable should be called with true")
    assert.are.equal(buf, enable_calls[1].opts.bufnr, "enable should target the attached buffer")
    assert.are.equal(
      0,
      #refresh_calls,
      "ftplugin must not call deprecated vim.lsp.codelens.refresh when enable exists"
    )
  end)

  it("registers its LspAttach handler only once when the filetype is set repeatedly", function()
    -- Neovim re-sources ftplugins on every FileType event and unlets the
    -- builtin `b:did_ftplugin` guard each time, so without a plugin-owned guard
    -- the handler (and thus code lens activation) would be registered twice.
    local enable_calls = {}
    vim.lsp.codelens.enable = function(on, opts)
      table.insert(enable_calls, { on = on, opts = opts })
    end

    local buf = make_python_buffer()
    -- Force a second FileType event for the same buffer.
    vim.bo[buf].filetype = "python"

    local handlers = vim.api.nvim_get_autocmds({ event = "LspAttach", buffer = buf })
    assert.are.equal(1, #handlers, "ftplugin should register exactly one LspAttach handler per buffer")

    vim.api.nvim_exec_autocmds("LspAttach", {
      buffer = buf,
      data = { client_id = 1 },
    })

    assert.are.equal(1, #enable_calls, "code lens should be activated exactly once despite repeated FileType events")
  end)

  it("falls back to vim.lsp.codelens.refresh on Neovim 0.10/0.11 where enable is absent", function()
    -- Simulate an older runtime where enable() does not exist.
    local refresh_calls = {}
    vim.lsp.codelens.enable = nil
    vim.lsp.codelens.refresh = function(opts)
      table.insert(refresh_calls, { opts = opts })
    end

    local buf = make_python_buffer()

    vim.api.nvim_exec_autocmds("LspAttach", {
      buffer = buf,
      data = { client_id = 1 },
    })

    assert.is_true(#refresh_calls >= 1, "ftplugin should fall back to refresh when enable is absent")
    assert.are.equal(buf, refresh_calls[1].opts.bufnr, "refresh should target the attached buffer")
  end)
end)
