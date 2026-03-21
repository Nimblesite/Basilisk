--- Generate luacov coverage report using neovim's LuaJIT.
---
--- Parses the luacov stats file directly and computes coverage for
--- basilisk source files (lua/basilisk/**), bypassing luacov's
--- reporter module to avoid include/exclude pattern matching issues
--- across different environments.
---
--- Usage: nvim --headless --noplugin -l tests/generate_report.lua

-- Also attempt to generate the standard luacov report for human reading.
local function try_luacov_report()
  local luarocks_path = vim.fn.trim(vim.fn.system("luarocks path --lr-path 2>/dev/null"))
  local luarocks_cpath = vim.fn.trim(vim.fn.system("luarocks path --lr-cpath 2>/dev/null"))
  if luarocks_path ~= "" then
    package.path = package.path .. ";" .. luarocks_path
  end
  if luarocks_cpath ~= "" then
    package.cpath = package.cpath .. ";" .. luarocks_cpath
  end
  local ok, reporter = pcall(require, "luacov.reporter")
  if ok then
    pcall(reporter.report)
  end
end

--- Check if a filename is a basilisk source file (not test, not external).
---@param filename string
---@return boolean
local function is_basilisk_source(filename)
  -- Must contain lua/basilisk/ (handles both absolute and relative paths).
  if not filename:find("lua/basilisk/") then
    return false
  end
  -- Must end in .lua (reject truncated paths like "profiling.l").
  if not filename:find("%.lua$") then
    return false
  end
  -- Exclude test files and minimal_init.
  if filename:find("tests/") or filename:find("minimal_init") then
    return false
  end
  return true
end

--- Parse luacov stats file and compute coverage for basilisk source files.
---@param statsfile string
---@return number total_hits
---@return number total_missed
---@return table<string, {hits: number, missed: number}> per-file stats
local function compute_coverage(statsfile)
  local fd = io.open(statsfile, "r")
  if not fd then
    return 0, 0, {}
  end

  local file_stats = {}
  local total_hits = 0
  local total_missed = 0

  while true do
    local max = fd:read("*n")
    if not max then break end
    if fd:read(1) ~= ":" then break end
    local filename = fd:read("*l")
    if not filename then break end

    local hits, missed = 0, 0
    for _ = 1, max do
      local count = fd:read("*n")
      if not count then break end
      if fd:read(1) ~= " " then break end
      if count > 0 then
        hits = hits + 1
      else
        missed = missed + 1
      end
    end

    if is_basilisk_source(filename) then
      -- Use just the basename for display.
      local basename = filename:match("[^/]+$") or filename
      file_stats[basename] = { hits = hits, missed = missed }
      total_hits = total_hits + hits
      total_missed = total_missed + missed
    end
  end

  fd:close()
  return total_hits, total_missed, file_stats
end

--- Write the coverage report to luacov.report.out.
---@param total_hits number
---@param total_missed number
---@param file_stats table<string, {hits: number, missed: number}>
local function write_report(total_hits, total_missed, file_stats)
  local fd = io.open("luacov.report.out", "w")
  if not fd then
    io.stderr:write("ERROR: cannot write luacov.report.out\n")
    return
  end

  fd:write(string.rep("=", 78) .. "\n")
  fd:write("Summary\n")
  fd:write(string.rep("=", 78) .. "\n")
  fd:write("\n")

  local hdr_fmt = "%-60s %5s %5s %8s\n"
  local fmt = "%-60s %5d %5d %8s\n"
  fd:write(string.format(hdr_fmt, "File", "Hits", "Missed", "Coverage"))
  fd:write(string.rep("-", 85) .. "\n")

  -- Sort by filename for stable output.
  local names = {}
  for name in pairs(file_stats) do
    names[#names + 1] = name
  end
  table.sort(names)

  for _, name in ipairs(names) do
    local st = file_stats[name]
    local total_lines = st.hits + st.missed
    local pct = total_lines > 0 and (st.hits / total_lines * 100) or 0
    fd:write(string.format(fmt, name, st.hits, st.missed, string.format("%.2f%%", pct)))
  end

  fd:write(string.rep("-", 85) .. "\n")
  local total_lines = total_hits + total_missed
  local pct = total_lines > 0 and (total_hits / total_lines * 100) or 0
  fd:write(string.format(fmt, "Total", total_hits, total_missed, string.format("%.2f%%", pct)))

  fd:close()
end

-- Main.
local cwd = vim.fn.getcwd()
io.stderr:write("  generate_report: cwd=" .. cwd .. "\n")

local statsfile = cwd .. "/luacov.stats.out"
if vim.fn.filereadable("luacov.stats.out") ~= 1 then
  io.stderr:write("  generate_report: luacov.stats.out not found\n")
  os.exit(1)
end

local stat_size = vim.fn.getfsize("luacov.stats.out")
io.stderr:write("  generate_report: luacov.stats.out size=" .. tostring(stat_size) .. " bytes\n")

local total_hits, total_missed, file_stats = compute_coverage(statsfile)
local file_count = 0
for _ in pairs(file_stats) do file_count = file_count + 1 end
io.stderr:write("  generate_report: matched " .. file_count .. " basilisk source files\n")

-- Try the standard luacov report first (for human reading if it works).
try_luacov_report()

-- Overwrite with our own report that we know works cross-platform.
write_report(total_hits, total_missed, file_stats)

os.exit(0)
