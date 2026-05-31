--- Filetype plugin for Python files.
---
--- Auto-loaded by Neovim when a Python buffer opens.
--- Sets up keymaps, inlay hints, and code lens for Basilisk.

-- Guard: only run if basilisk was set up.
local ok, basilisk = pcall(require, "basilisk")
if not ok or not basilisk.config then
  return
end

-- Guard: source once per buffer. Neovim re-runs ftplugins on every FileType
-- event and unlets the builtin `b:did_ftplugin` before ours runs (we are
-- earlier on the runtimepath), so we keep our own buffer-local marker to avoid
-- registering the LspAttach handler more than once.
if vim.b.basilisk_did_ftplugin then
  return
end
vim.b.basilisk_did_ftplugin = true

local config = basilisk.config

-- Set up keymaps on LspAttach for basilisk clients only.
vim.api.nvim_create_autocmd("LspAttach", {
  buffer = 0,
  callback = function(args)
    local client = vim.lsp.get_client_by_id(args.data.client_id)
    if not client or client.name ~= "basilisk" then
      return
    end

    -- Enable inlay hints if supported.
    if client:supports_method("textDocument/inlayHint") then
      vim.lsp.inlay_hint.enable(true, { bufnr = args.buf })
    end

    -- Enable code lens if supported. Activation is version-compatible: see
    -- basilisk.codelens (prefers vim.lsp.codelens.enable on Neovim 0.12+, which
    -- replaced the deprecated/removed refresh()).
    if client:supports_method("textDocument/codeLens") then
      require("basilisk.codelens").activate(args.buf)
    end

    -- Skip keymaps if disabled.
    if not config.keymaps.enabled then
      return
    end

    local buf = args.buf
    local map = function(mode, lhs, rhs, desc)
      vim.keymap.set(mode, lhs, rhs, { buffer = buf, desc = desc })
    end

    -- Standard LSP keymaps.
    map("n", "gd", vim.lsp.buf.definition, "Go to definition")
    map("n", "gD", vim.lsp.buf.declaration, "Go to declaration")
    map("n", "gy", vim.lsp.buf.type_definition, "Go to type definition")
    map("n", "gr", vim.lsp.buf.references, "Find references")
    map("n", "K", vim.lsp.buf.hover, "Hover")
    map("i", "<C-k>", vim.lsp.buf.signature_help, "Signature help")
    map("n", "<leader>rn", vim.lsp.buf.rename, "Rename")
    map("n", "<leader>ca", vim.lsp.buf.code_action, "Code action")

    -- Basilisk-specific keymaps with configurable prefix.
    local prefix = config.keymaps.prefix
    map("n", prefix .. "r", "<cmd>BasiliskRestart<CR>", "Restart server")
    map("n", prefix .. "o", "<cmd>BasiliskOrganizeImports<CR>", "Organize imports")
    map("n", prefix .. "p", "<cmd>BasiliskProfile<CR>", "Start profiling")
    map("n", prefix .. "P", "<cmd>BasiliskProfileStop<CR>", "Stop profiling")
    map("n", prefix .. "m", "<cmd>BasiliskMemLeak<CR>", "Start memory tracking")
    map("n", prefix .. "M", "<cmd>BasiliskMemStop<CR>", "Stop memory tracking")
    map("n", prefix .. "t", "<cmd>BasiliskTestToggle<CR>", "Toggle test explorer")
    map("n", prefix .. "d", "<cmd>BasiliskDebugFile<CR>", "Debug current file")
    map("n", prefix .. "R", "<cmd>BasiliskTestRun<CR>", "Run test at cursor")
    map("n", prefix .. "D", "<cmd>BasiliskTestDebug<CR>", "Debug test at cursor")

    -- Refactoring keymaps.
    map("v", prefix .. "ev", "<cmd>BasiliskExtractVariable<CR>", "Extract variable")
    map("v", prefix .. "ec", "<cmd>BasiliskExtractConstant<CR>", "Extract constant")
    map("n", prefix .. "cu", "<cmd>BasiliskConvertUnion<CR>", "Convert Union/Optional")
    map("n", prefix .. "im", "<cmd>BasiliskImplementMethods<CR>", "Implement abstract methods")
    map("n", prefix .. "fa", "<cmd>BasiliskFixFile<CR>", "Fix all in file")
  end,
})
