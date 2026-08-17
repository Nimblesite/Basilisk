--- Tests for after/lsp/basilisk.lua — the native vim.lsp.config fallback for
--- users who never call require("basilisk").setup()
--- ([NVIM-LSP-CLIENT-CONFIGURATION-FALLBACK]).
---
--- Neovim's built-in LSP loader requires this file to evaluate to a table.
--- Returning nil when no binary resolves surfaces to the user as
--- "after/lsp/basilisk.lua: not a table" — issue #370, symptom 3.

describe("after/lsp/basilisk.lua", function()
  local binary = require("basilisk.binary")

  --- Absolute path to the file under test (…/basilisk.nvim/after/lsp/basilisk.lua).
  local function config_file()
    local spec = debug.getinfo(1, "S").source:sub(2)
    return vim.fn.fnamemodify(spec, ":h:h:h") .. "/after/lsp/basilisk.lua"
  end

  --- Evaluate the file with `binary.resolve` forced to `resolved`.
  ---@param resolved string?
  ---@return any
  local function load_with(resolved)
    local original = binary.resolve
    binary.resolve = function()
      return resolved
    end
    local ok, result = pcall(dofile, config_file())
    binary.resolve = original
    assert(ok, tostring(result))
    return result
  end

  it("returns a table even when no binary resolves (issue #370)", function()
    local config = load_with(nil)

    assert.are.equal(
      "table",
      type(config),
      "Neovim's LSP loader errors with 'not a table' on anything else"
    )
  end)

  it("still declares a runnable command when no binary resolves", function()
    local config = load_with(nil)

    assert.are.equal("table", type(config.cmd), "cmd must survive an unresolved binary")
    assert.are.equal("basilisk", vim.fn.fnamemodify(config.cmd[1], ":t"))
    assert.are.equal("lsp", config.cmd[2])
    assert.are.same({ "python" }, config.filetypes)
  end)

  it("uses the resolved binary when one is found", function()
    local config = load_with("/opt/basilisk/bin/basilisk")

    assert.are.same({ "/opt/basilisk/bin/basilisk", "lsp" }, config.cmd)
  end)
end)
