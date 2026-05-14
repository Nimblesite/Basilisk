--- Binary resolution for the basilisk executable.
---
--- Follows the cascade defined in LSP-SPEC.md:
--- 1. User-configured path (editor setting)
--- 2. BASILISK_PATH environment variable
--- 3. ~/.cargo/bin/basilisk
--- 4. /usr/local/bin/basilisk
--- 5. /opt/homebrew/bin/basilisk
--- 6. Fall back to OS PATH search
--- 7. Auto-download from GitHub releases (fallback)

local log = require("basilisk.log")

local M = {}

--- GitHub repo for release downloads.
local GITHUB_REPO = "Nimblesite/Basilisk"

--- GitHub API URL for latest release.
local RELEASES_API = "https://api.github.com/repos/" .. GITHUB_REPO .. "/releases/latest"

--- Directory where downloaded binaries are cached.
---@return string
local function download_dir()
  return vim.fn.stdpath("data") .. "/basilisk"
end

--- Check whether a file exists and is executable.
---@param path string
---@return boolean
local function is_executable(path)
  return vim.fn.executable(path) == 1
end

--- Check whether a configured binary path is usable.
---@param path? string
---@return boolean
function M.is_executable(path)
  return type(path) == "string" and path ~= "" and is_executable(path)
end

--- Parse a semver-ish string into (major, minor, patch).
--- Strips leading "v" and "basilisk " prefix.
---@param version_str string
---@return integer, integer, integer
local function parse_semver(version_str)
  local stripped = version_str:gsub("^basilisk%s+", ""):gsub("^v", "")
  local major, minor, patch = stripped:match("^(%d+)%.(%d+)%.(%d+)")
  return tonumber(major) or 0, tonumber(minor) or 0, tonumber(patch) or 0
end

--- Compare two version strings. Returns true if latest is newer than current.
---@param current string
---@param latest string
---@return boolean
function M.is_newer_version(current, latest)
  local cur_maj, cur_min, cur_pat = parse_semver(current)
  local lat_maj, lat_min, lat_pat = parse_semver(latest)
  if lat_maj ~= cur_maj then return lat_maj > cur_maj end
  if lat_min ~= cur_min then return lat_min > cur_min end
  return lat_pat > cur_pat
end

--- Detect the platform-specific asset name for GitHub releases.
---@return string? asset_name, boolean is_windows
function M.platform_asset_name()
  local uname = vim.uv.os_uname()
  local sysname = uname.sysname:lower()
  local machine = uname.machine:lower()

  local os_str
  if sysname == "darwin" then
    os_str = "apple-darwin"
  elseif sysname == "linux" then
    os_str = "unknown-linux-gnu"
  elseif sysname:find("windows") or sysname:find("mingw") then
    os_str = "pc-windows-msvc"
  else
    return nil, false
  end

  local arch_str
  if machine == "arm64" or machine == "aarch64" then
    arch_str = "aarch64"
  elseif machine == "x86_64" or machine == "amd64" then
    arch_str = "x86_64"
  else
    return nil, false
  end

  local is_windows = os_str == "pc-windows-msvc"
  local ext = is_windows and "zip" or "tar.gz"
  return string.format("basilisk-%s-%s.%s", arch_str, os_str, ext), is_windows
end

--- Fetch the latest release info from GitHub (synchronous, via curl).
---@return table? release { tag_name: string, assets: [{name, browser_download_url}] }
function M.fetch_latest_release()
  local ok, result = pcall(vim.fn.system, {
    "curl", "-sSL",
    "-H", "Accept: application/vnd.github+json",
    RELEASES_API,
  })
  if not ok or vim.v.shell_error ~= 0 then
    return nil
  end
  local decode_ok, data = pcall(vim.json.decode, result)
  if not decode_ok or type(data) ~= "table" or not data.tag_name then
    return nil
  end
  return data
end

--- Download the basilisk binary from the latest GitHub release.
--- Returns the path to the downloaded binary, or nil on failure.
---@return string? path, string? version
function M.download()
  local release = M.fetch_latest_release()
  if not release then
    return nil, nil
  end

  local asset_name, is_windows = M.platform_asset_name()
  if not asset_name then
    return nil, nil
  end

  local download_url
  for _, asset in ipairs(release.assets or {}) do
    if asset.name == asset_name then
      download_url = asset.browser_download_url
      break
    end
  end
  if not download_url then
    return nil, nil
  end

  local version = release.tag_name
  local dir = download_dir() .. "/" .. version
  local binary_name = is_windows and "basilisk.exe" or "basilisk"
  local binary_path = dir .. "/" .. binary_name

  -- Already downloaded.
  if is_executable(binary_path) then
    return binary_path, version
  end

  vim.fn.mkdir(dir, "p")

  local archive_path = dir .. "/" .. asset_name
  log.info("downloading %s...", version)

  local curl_ok = pcall(vim.fn.system, {
    "curl", "-sSL", "-o", archive_path, download_url,
  })
  if not curl_ok or vim.v.shell_error ~= 0 then
    log.error("download failed")
    return nil, nil
  end

  -- Extract.
  if is_windows then
    pcall(vim.fn.system, { "unzip", "-o", archive_path, "-d", dir })
  else
    pcall(vim.fn.system, { "tar", "xzf", archive_path, "-C", dir })
  end

  if vim.v.shell_error ~= 0 then
    log.error("extraction failed")
    return nil, nil
  end

  -- Clean up archive.
  vim.fn.delete(archive_path)

  -- Make executable.
  if not is_windows then
    vim.fn.setfperm(binary_path, "rwxr-xr-x")
  end

  if is_executable(binary_path) then
    log.info("installed %s", version)
    return binary_path, version
  end

  return nil, nil
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
    log.warn("configured binary_path not found: %s", configured_path)
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

  -- 7. Auto-download from GitHub releases.
  local downloaded_path = M.download()
  if downloaded_path then
    return downloaded_path
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

--- Check if a newer version is available and notify the user.
--- Non-blocking: runs curl in the background via vim.system.
---@param binary_path string
function M.check_for_updates(binary_path)
  local current_version = M.version(binary_path)
  if not current_version then
    return
  end

  vim.system(
    { "curl", "-sSL", "-H", "Accept: application/vnd.github+json", RELEASES_API },
    { text = true },
    function(result)
      if result.code ~= 0 or not result.stdout then
        return
      end
      local decode_ok, data = pcall(vim.json.decode, result.stdout)
      if not decode_ok or type(data) ~= "table" or not data.tag_name then
        return
      end
      if M.is_newer_version(current_version, data.tag_name) then
        vim.schedule(function()
          log.info(
            "update available: %s → %s. Run :checkhealth basilisk for details.",
            current_version,
            data.tag_name
          )
        end)
      end
    end
  )
end

return M
