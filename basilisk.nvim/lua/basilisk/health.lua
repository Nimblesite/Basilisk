--- Health check for :checkhealth basilisk.
---
--- Reports on binary availability, Python interpreter,
--- optional integrations, and configuration summary.

local binary = require("basilisk.binary")

local M = {}

--- Implements [NVIM-HEALTH-CHECK] — :checkhealth basilisk reports Neovim version,
--- the basilisk binary + version, Python, and the optional debugpy/nvim-dap/
--- nvim-dap-ui/ruff integrations (plus uv and a config summary).
function M.check()
  vim.health.start("basilisk.nvim")

  -- Neovim version.
  if vim.fn.has("nvim-0.10") == 1 then
    vim.health.ok("Neovim >= 0.10")
  else
    vim.health.error("Neovim >= 0.10 required", { "Upgrade Neovim to 0.10 or later." })
  end

  -- Basilisk binary. Forward the configured binary_path so the cascade's
  -- first step (setup({ binary_path = ... })) is honored — otherwise a binary
  -- reachable only via config is falsely reported as not found (issue #67).
  local cfg_ok, basilisk_cfg = pcall(require, "basilisk")
  local configured_path = cfg_ok and basilisk_cfg.config and basilisk_cfg.config.binary_path or nil
  local bin = binary.resolve(configured_path)
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

  -- uv (optional).
  local uv = vim.fn.exepath("uv")
  if uv ~= "" then
    local uv_ver = vim.trim(vim.fn.system({ uv, "--version" }))
    vim.health.ok("uv found: " .. uv .. " (" .. uv_ver .. ")")
  else
    vim.health.info("uv not found (optional, for package management)")
  end

  -- Configuration summary.
  vim.health.start("basilisk.nvim configuration")
  local basilisk_ok, basilisk = pcall(require, "basilisk")
  if basilisk_ok and basilisk.config then
    local cfg = basilisk.config
    vim.health.ok("Analysis mode: " .. cfg.analysis_mode)
    vim.health.ok("Ruff: " .. (cfg.ruff.enabled and "enabled" or "disabled"))
    vim.health.ok("Debugger: " .. (cfg.debugger.enabled and "enabled" or "disabled"))
    vim.health.ok("Test explorer: " .. (cfg.test_explorer.enabled and "enabled" or "disabled"))
    vim.health.ok("uv integration: " .. (cfg.uv.enabled and "enabled" or "disabled"))
    vim.health.ok("Keymaps: " .. (cfg.keymaps.enabled and "enabled" or "disabled"))
    vim.health.ok("Log level: " .. cfg.log_level)
  else
    vim.health.info("basilisk.setup() has not been called yet")
  end
end

return M
