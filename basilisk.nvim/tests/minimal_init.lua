--- Minimal init for plenary.nvim tests.
---
--- Usage: nvim --headless -u tests/minimal_init.lua -c "PlenaryBustedDirectory tests/basilisk"
--- Coverage: LUACOV=1 nvim --headless -u tests/minimal_init.lua ...

-- Add this plugin to the runtime path.
local plugin_dir = vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":h:h")
vim.opt.rtp:prepend(plugin_dir)

-- Start code coverage if LUACOV env var is set.
if os.getenv("LUACOV") then
  -- Disable JIT so debug hooks work for coverage tracking.
  if jit then
    jit.off()
  end
  -- Disable vim.loader bytecode cache so luacov's debug hook can see
  -- source files. Without this, cached modules bypass the line hook
  -- and produce 0% coverage for all basilisk sources.
  if vim.loader and vim.loader.reset then
    vim.loader.reset()
    -- vim.loader.enable(false) disables the cache (Neovim 0.11+).
    -- Older builds expose vim.loader.disable() instead.
    if vim.loader.enable then
      vim.loader.enable(false)
    elseif vim.loader.disable then
      vim.loader.disable()
    end
  end
  -- Add luarocks paths so we can find luacov.
  local luarocks_path = vim.fn.trim(vim.fn.system("luarocks path --lr-path 2>/dev/null"))
  local luarocks_cpath = vim.fn.trim(vim.fn.system("luarocks path --lr-cpath 2>/dev/null"))
  if luarocks_path ~= "" then
    package.path = package.path .. ";" .. luarocks_path
  end
  if luarocks_cpath ~= "" then
    package.cpath = package.cpath .. ";" .. luarocks_cpath
  end
  -- Clear any cached basilisk modules so they get loaded AFTER the
  -- debug hook is installed, ensuring coverage tracking.
  for mod_name in pairs(package.loaded) do
    if mod_name:match("^basilisk") then
      package.loaded[mod_name] = nil
    end
  end

  local ok, runner = pcall(require, "luacov.runner")
  if ok then
    runner.init({
      configfile = plugin_dir .. "/.luacov",
      tick = true,
    })
    -- Flush coverage data on VimLeave so headless runs don't lose data.
    vim.api.nvim_create_autocmd("VimLeavePre", {
      callback = function()
        runner.save_stats()
        runner.shutdown()
      end,
    })
  end
end

-- Add plenary.nvim if available in parent or standard locations.
local plenary_paths = {
  "/tmp/plenary.nvim",
  vim.fn.expand("~/.local/share/nvim/lazy/plenary.nvim"),
  vim.fn.expand("~/.local/share/nvim/site/pack/vendor/start/plenary.nvim"),
  vim.fn.stdpath("data") .. "/lazy/plenary.nvim",
  "../plenary.nvim",
}
for _, path in ipairs(plenary_paths) do
  if vim.fn.isdirectory(path) == 1 then
    vim.opt.rtp:prepend(path)
    break
  end
end

-- Add mini.test if available.
local mini_paths = {
  "/tmp/mini.nvim",
  vim.fn.expand("~/.local/share/nvim/lazy/mini.nvim"),
  vim.fn.expand("~/.local/share/nvim/lazy/mini.test"),
  vim.fn.stdpath("data") .. "/lazy/mini.nvim",
}
for _, path in ipairs(mini_paths) do
  if vim.fn.isdirectory(path) == 1 then
    vim.opt.rtp:prepend(path)
    break
  end
end

-- Add nvim-dap if available.
local dap_paths = {
  "/tmp/nvim-dap",
  vim.fn.expand("~/.local/share/nvim/lazy/nvim-dap"),
  vim.fn.stdpath("data") .. "/lazy/nvim-dap",
}
for _, path in ipairs(dap_paths) do
  if vim.fn.isdirectory(path) == 1 then
    vim.opt.rtp:prepend(path)
    break
  end
end

-- Minimal settings.
vim.o.swapfile = false
vim.o.backup = false
vim.o.writebackup = false
vim.cmd("filetype plugin indent on")
vim.cmd("syntax enable")
