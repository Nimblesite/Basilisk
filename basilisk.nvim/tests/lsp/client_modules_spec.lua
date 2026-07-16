--- Client-local module tests, run inside the LSP e2e gate so their lines
--- count toward the enforced Lua coverage threshold (scripts/test-nvim.sh).
---
--- Tests [NVIM-NEOVIM-ONLY-CONFIGURATION] (config validation/resolution),
--- [ANALYSIS-OPEN] (tab tracking setup for openFilesOnly), and the Code Lens
--- row of [NVIM-LSP-CLIENT-CONFIGURATION-API-MAPPINGS] (version-gated
--- activation). No server needed: everything here is client-side behaviour,
--- so no binary guard — these run on every matrix leg.

local codelens = require("basilisk.codelens")
local config = require("basilisk.config")
local tab_tracking = require("basilisk.tab_tracking")

local function messages_of(errors)
  return table.concat(errors, "\n")
end

describe("config.validate [NVIM-NEOVIM-ONLY-CONFIGURATION]", function()
  it("accepts the shipped defaults", function()
    assert.same({}, config.validate(config.defaults))
  end)

  it("names every invalid enum value", function()
    local bad = vim.tbl_deep_extend("force", {}, config.defaults, {
      analysis_mode = "everything",
      test_explorer = { framework = "nose", position = "top" },
      log_level = "loud",
    })
    local errors = config.validate(bad)
    assert.equals(4, #errors)
    local all = messages_of(errors)
    assert.truthy(all:find("invalid analysis_mode: everything", 1, true))
    assert.truthy(all:find("invalid test_explorer.framework: nose", 1, true))
    assert.truthy(all:find("invalid test_explorer.position: top", 1, true))
    assert.truthy(all:find("invalid log_level: loud", 1, true))
  end)
end)

describe("config.resolve [NVIM-NEOVIM-ONLY-CONFIGURATION]", function()
  it("returns the defaults when called with no opts", function()
    assert.same(config.defaults, config.resolve())
  end)

  it("deep-merges user opts over the defaults", function()
    local resolved = config.resolve({
      analysis_mode = "openFilesOnly",
      keymaps = { prefix = "<leader>x" },
    })
    assert.equals("openFilesOnly", resolved.analysis_mode)
    assert.equals("<leader>x", resolved.keymaps.prefix)
    -- Sibling keys of a partially-overridden table keep their defaults.
    assert.is_true(resolved.keymaps.enabled)
    assert.equals("ruff", resolved.formatter)
  end)

  it("logs but does not reject an invalid value", function()
    -- Validation errors are reported through the log ([NVIM-HEALTH-CHECK]
    -- surfaces them); resolve still returns the merged config unchanged.
    local resolved = config.resolve({ log_level = "shout" })
    assert.equals("shout", resolved.log_level)
  end)
end)

describe("tab_tracking.setup [ANALYSIS-OPEN]", function()
  local function tracking_autocmds()
    local ok, autocmds = pcall(vim.api.nvim_get_autocmds, { group = "BasiliskTabTracking" })
    if not ok then
      return nil
    end
    return autocmds
  end

  it("is inert outside openFilesOnly mode", function()
    tab_tracking.setup(config.resolve({ analysis_mode = "wholeModule" }))
    assert.is_nil(tracking_autocmds())
  end)

  it("registers hidden-buffer tracking in openFilesOnly mode", function()
    -- A visible python buffer seeds the known-open set via the same
    -- collect path the autocmd uses.
    vim.cmd.edit("tracked_visible.py")
    vim.bo.filetype = "python"

    tab_tracking.setup(config.resolve({ analysis_mode = "openFilesOnly" }))

    local autocmds = tracking_autocmds()
    assert.is_table(autocmds)
    local events = {}
    for _, autocmd in ipairs(autocmds or {}) do
      events[autocmd.event] = true
      assert.equals("*.py", autocmd.pattern)
    end
    assert.is_true(events.BufHidden)
    assert.is_true(events.WinClosed)
    assert.is_true(events.BufDelete)

    vim.api.nvim_del_augroup_by_name("BasiliskTabTracking")
    vim.cmd.bwipeout({ bang = true })
  end)
end)

describe("codelens.activate [NVIM-LSP-CLIENT-CONFIGURATION-API-MAPPINGS]", function()
  it("activates through the API the runtime exposes", function()
    local buf = vim.api.nvim_create_buf(false, true)
    codelens.activate(buf)
    if vim.lsp.codelens.enable then
      -- 0.12+ path: enable() owns refresh; report its own view when the
      -- runtime can be asked.
      if vim.lsp.codelens.is_enabled then
        assert.is_true(vim.lsp.codelens.is_enabled({ bufnr = buf }))
      end
    else
      -- 0.10/0.11 fallback: a manual refresh loop is installed.
      local autocmds = vim.api.nvim_get_autocmds({
        event = { "BufEnter", "InsertLeave" },
        buffer = buf,
      })
      assert.is_true(#autocmds >= 2)
    end
    vim.api.nvim_buf_delete(buf, { force = true })
  end)
end)
