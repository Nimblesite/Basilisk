--- Binary resolution for the basilisk executable.
---
--- Follows the cascade defined in LSP-SPEC.md:
--- 1. User-configured path (editor setting)
--- 2. BASILISK_PATH environment variable
--- 3. ~/.cargo/bin/basilisk
--- 4. /usr/local/bin/basilisk
--- 5. /opt/homebrew/bin/basilisk
--- 6. Fall back to OS PATH search

local M = {}

--- Check whether a file exists and is executable.
---@param path string
---@return boolean
local function is_executable(path)
  return vim.fn.executable(path) == 1
end

--- Resolve the basilisk binary path using the LSP-SPEC cascade.
---@param configured_path? string User-configured path from setup().
---@return string? path Absolute path to the binary, or nil if not found.
function M.resolve(configured_path)
  -- 1. User-configured path.
  if configured_path and configured_path ~= "" then
    if is_executable(configured_path) then
      return configured_path
    end
    vim.notify(
      "[basilisk] configured binary_path not found: " .. configured_path,
      vim.log.levels.WARN
    )
  end

  -- 2. BASILISK_PATH environment variable.
  local env_path = vim.env.BASILISK_PATH
  if env_path and env_path ~= "" and is_executable(env_path) then
    return env_path
  end

  -- 3-5. Well-known locations.
  local candidates = {
    vim.fn.expand("~/.cargo/bin/basilisk"),
    "/usr/local/bin/basilisk",
    "/opt/homebrew/bin/basilisk",
  }
  for _, candidate in ipairs(candidates) do
    if is_executable(candidate) then
      return candidate
    end
  end

  -- 6. OS PATH search.
  local on_path = vim.fn.exepath("basilisk")
  if on_path ~= "" then
    return on_path
  end

  return nil
end

--- Get the version string from the binary.
---@param binary_path string
---@return string? version
function M.version(binary_path)
  if not is_executable(binary_path) then
    return nil
  end
  local ok, result = pcall(vim.fn.system, { binary_path, "--version" })
  if not ok or vim.v.shell_error ~= 0 then
    return nil
  end
  return vim.trim(result)
end

return M
