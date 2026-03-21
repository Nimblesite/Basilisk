--- Generate luacov coverage report using neovim's LuaJIT.
---
--- This ensures the same Lua runtime that collected the stats (via neovim)
--- is used to parse and generate the report, avoiding Lua 5.1/5.4
--- incompatibilities with the standalone luacov tool.
---
--- Usage: nvim --headless --noplugin -l tests/generate_report.lua

-- Add luarocks paths so we can find luacov.
local luarocks_path = vim.fn.trim(vim.fn.system("luarocks path --lr-path 2>/dev/null"))
local luarocks_cpath = vim.fn.trim(vim.fn.system("luarocks path --lr-cpath 2>/dev/null"))
if luarocks_path ~= "" then
  package.path = package.path .. ";" .. luarocks_path
end
if luarocks_cpath ~= "" then
  package.cpath = package.cpath .. ";" .. luarocks_cpath
end

local ok, reporter = pcall(require, "luacov.reporter")
if not ok then
  io.stderr:write("luacov.reporter not found: " .. tostring(reporter) .. "\n")
  os.exit(1)
end

reporter.report()
os.exit(0)
