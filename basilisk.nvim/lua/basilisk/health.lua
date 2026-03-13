--- Health check for :checkhealth basilisk.
---
--- Reports on binary availability, Python interpreter,
--- and optional integrations (debugpy, nvim-dap, ruff).

local binary = require("basilisk.binary")

local M = {}

function M.check()
  vim.health.start("basilisk.nvim")

  -- Neovim version.
  if vim.fn.has("nvim-0.10") == 1 then
    vim.health.ok("Neovim >= 0.10")
  else
    vim.health.error("Neovim >= 0.10 required", { "Upgrade Neovim to 0.10 or later." })
  end

  -- Basilisk binary.
  local bin = binary.resolve()
  if bin then
    local ver = binary.version(bin) or "unknown"
    vim.health.ok("basilisk binary found: " .. bin .. " (" .. ver .. ")")
  else
    vim.health.error("basilisk binary not found", {
      "Install with: cargo install basilisk-cli",
      "Or set vim.env.BASILISK_PATH",
    })
  end

  -- Python interpreter.
  local python = vim.fn.exepath("python3")
  if python == "" then
    python = vim.fn.exepath("python")
  end
  if python ~= "" then
    local py_ver = vim.trim(vim.fn.system({ python, "--version" }))
    vim.health.ok("Python found: " .. python .. " (" .. py_ver .. ")")
  else
    vim.health.warn("Python not found on PATH", {
      "Some features (debugging, testing) require Python.",
    })
  end

  -- debugpy (optional).
  if python ~= "" then
    local result = vim.fn.system({ python, "-c", "import debugpy; print(debugpy.__version__)" })
    if vim.v.shell_error == 0 then
      vim.health.ok("debugpy installed: " .. vim.trim(result))
    else
      vim.health.info("debugpy not installed (optional, for debugging)")
    end
  end

  -- nvim-dap (optional).
  local dap_ok = pcall(require, "dap")
  if dap_ok then
    vim.health.ok("nvim-dap available")
  else
    vim.health.info("nvim-dap not installed (optional, for debugging)")
  end

  -- nvim-dap-ui (optional).
  local dapui_ok = pcall(require, "dapui")
  if dapui_ok then
    vim.health.ok("nvim-dap-ui available")
  else
    vim.health.info("nvim-dap-ui not installed (optional, for debug UI)")
  end

  -- ruff (optional).
  local ruff = vim.fn.exepath("ruff")
  if ruff ~= "" then
    local ruff_ver = vim.trim(vim.fn.system({ ruff, "--version" }))
    vim.health.ok("ruff found: " .. ruff .. " (" .. ruff_ver .. ")")
  else
    vim.health.info("ruff not found (optional, for formatting)")
  end
end

return M
