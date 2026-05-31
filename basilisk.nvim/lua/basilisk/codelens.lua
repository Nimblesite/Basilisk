--- Version-compatible LSP code lens activation.
---
--- Centralizes the one correct way to turn on code lens for a buffer so the
--- ftplugin and tests never duplicate the version check. See the Code Lens row
--- in NEOVIM-SPEC.md §NVIM-LSP-CLIENT-CONFIGURATION-API-MAPPINGS.
---
--- `vim.lsp.codelens.enable` (Neovim 0.12+) installs its own debounced refresh
--- autocmds, so it is preferred whenever present. `vim.lsp.codelens.refresh` is
--- deprecated on 0.12 and removed on 0.13; it is only used as a fallback on
--- Neovim 0.10/0.11, paired with a manual BufEnter/InsertLeave refresh loop.

local M = {}

--- Activate code lens for a buffer using the best API the runtime exposes.
---@param bufnr integer
function M.activate(bufnr)
  if vim.lsp.codelens.enable then
    vim.lsp.codelens.enable(true, { bufnr = bufnr })
    return
  end

  vim.lsp.codelens.refresh({ bufnr = bufnr })
  vim.api.nvim_create_autocmd({ "BufEnter", "InsertLeave" }, {
    buffer = bufnr,
    callback = function()
      vim.lsp.codelens.refresh({ bufnr = bufnr })
    end,
  })
end

return M
