--- Tests for basilisk.codelens module.
---
--- Pins [NVIM-LSP-CLIENT-CONFIGURATION-API-MAPPINGS] (Code Lens row): the
--- plugin must activate code lens through `vim.lsp.codelens.enable` whenever the
--- runtime exposes it (Neovim 0.12+, which installs its own debounced refresh),
--- and fall back to `refresh()` plus a manual BufEnter/InsertLeave loop only on
--- 0.10/0.11 — `refresh()` is deprecated on 0.12 and removed on 0.13, so calling
--- it on a modern runtime is a deprecation warning today and a break tomorrow.
---
--- Both branches are exercised on ONE Neovim by swapping the `vim.lsp.codelens`
--- table, so the version the tests happen to run on never decides which half of
--- the contract is checked.

describe("basilisk.codelens", function()
  local codelens = require("basilisk.codelens")

  local original
  local calls

  before_each(function()
    original = vim.lsp.codelens
    calls = { enable = {}, refresh = {} }
  end)

  after_each(function()
    vim.lsp.codelens = original
  end)

  --- Install a stub `vim.lsp.codelens` recording its calls. `with_enable`
  --- decides whether the modern API appears to exist.
  local function stub_codelens(with_enable)
    local stub = {
      refresh = function(opts)
        table.insert(calls.refresh, opts)
      end,
    }
    if with_enable then
      stub.enable = function(on, opts)
        table.insert(calls.enable, { on = on, opts = opts })
      end
    end
    vim.lsp.codelens = stub
  end

  describe("activate on a runtime with vim.lsp.codelens.enable", function()
    it("enables code lens for the buffer and never calls the deprecated refresh", function()
      local bufnr = vim.api.nvim_create_buf(false, true)
      stub_codelens(true)

      codelens.activate(bufnr)

      assert.equals(1, #calls.enable, "must enable code lens exactly once")
      assert.is_true(calls.enable[1].on, "must enable, not disable")
      assert.equals(bufnr, calls.enable[1].opts.bufnr, "must target the given buffer")
      assert.equals(0, #calls.refresh, "refresh() is deprecated on 0.12+ and must not be called")

      vim.api.nvim_buf_delete(bufnr, { force = true })
    end)

    it("registers no refresh autocmds — the API installs its own", function()
      local bufnr = vim.api.nvim_create_buf(false, true)
      stub_codelens(true)

      codelens.activate(bufnr)
      local autocmds = vim.api.nvim_get_autocmds({
        event = { "BufEnter", "InsertLeave" },
        buffer = bufnr,
      })

      assert.equals(0, #autocmds, "duplicating the built-in refresh loop would double-request lenses")

      vim.api.nvim_buf_delete(bufnr, { force = true })
    end)
  end)

  describe("activate on a runtime without vim.lsp.codelens.enable", function()
    it("refreshes immediately for the buffer", function()
      local bufnr = vim.api.nvim_create_buf(false, true)
      stub_codelens(false)

      codelens.activate(bufnr)

      assert.is_true(#calls.refresh >= 1, "0.10/0.11 must get an initial refresh")
      assert.equals(bufnr, calls.refresh[1].bufnr, "must refresh the given buffer")

      vim.api.nvim_buf_delete(bufnr, { force = true })
    end)

    it("keeps lenses current by refreshing on BufEnter and InsertLeave", function()
      local bufnr = vim.api.nvim_create_buf(false, true)
      stub_codelens(false)

      codelens.activate(bufnr)
      local before = #calls.refresh
      vim.api.nvim_exec_autocmds("BufEnter", { buffer = bufnr })
      vim.api.nvim_exec_autocmds("InsertLeave", { buffer = bufnr })

      assert.equals(before + 2, #calls.refresh, "both events must re-request lenses")
      assert.equals(bufnr, calls.refresh[#calls.refresh].bufnr, "every refresh stays buffer-scoped")

      vim.api.nvim_buf_delete(bufnr, { force = true })
    end)

    it("scopes its autocmds to the buffer it was given", function()
      local bufnr = vim.api.nvim_create_buf(false, true)
      local other = vim.api.nvim_create_buf(false, true)
      stub_codelens(false)

      codelens.activate(bufnr)
      local before = #calls.refresh
      vim.api.nvim_exec_autocmds("BufEnter", { buffer = other })

      assert.equals(before, #calls.refresh, "another buffer's events must not refresh this one")

      vim.api.nvim_buf_delete(bufnr, { force = true })
      vim.api.nvim_buf_delete(other, { force = true })
    end)
  end)
end)
