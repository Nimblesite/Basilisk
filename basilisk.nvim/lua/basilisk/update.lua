--- In-editor install and upgrade of the basilisk binary.
---
--- Implements [NVIM-BINARY-UPGRADE] — the flows behind :BasiliskUpdate and
--- :BasiliskInstall. Reuses binary.download() (the resolve() step-7 engine);
--- the curl/extract logic is never duplicated here.

local binary = require("basilisk.binary")
local log = require("basilisk.log")

local M = {}

--- Refusal advice per install source ([NVIM-BINARY-UPGRADE-SOURCES]):
--- :BasiliskUpdate never clobbers a binary another tool owns — it steers the
--- user to that tool's own upgrade command instead.
local SOURCE_ADVICE = {
  dev = "resolved binary is a local dev build (0.0.0) — rebuild your checkout instead of overwriting it with a release",
  homebrew = "binary is managed by Homebrew — run `brew upgrade basilisk` instead",
  scoop = "binary is managed by Scoop — run `scoop update basilisk` instead",
  cargo = "binary was installed by cargo — run `cargo install basilisk-cli` instead",
}

--- Ask before touching the network, so the update notice has a real accept
--- step in the TUI ([NVIM-BINARY-UPGRADE-CONFIRM]).
---@param prompt string
---@param verb "Update"|"Install"
---@param on_accept fun()
local function confirm(prompt, verb, on_accept)
  local accept = verb .. " now"
  vim.ui.select({ accept, "Later" }, { prompt = prompt }, function(choice)
    if choice == accept then
      on_accept()
    end
  end)
end

--- Download the latest release into the managed cache, point the plugin
--- config at it, and restart the LSP client on the new binary.
---@param config BasiliskConfig
local function download_and_restart(config)
  local path, version = binary.download()
  if not path then
    log.error("download failed — check your network and :checkhealth basilisk")
    return
  end
  config.binary_path = path
  local plugin_ok, plugin = pcall(require, "basilisk")
  if plugin_ok and plugin.config and plugin.config ~= config then
    plugin.config.binary_path = path
  end
  local lsp = require("basilisk.lsp")
  lsp.reset_restart_count()
  lsp.restart(config, true)
  log.info("installed %s — restarting the LSP server", version)
end

--- :BasiliskUpdate — upgrade a plugin-managed (or manual) install to the
--- latest GitHub release. No-op when already current; refuses installs that
--- belong to a package manager or a dev checkout.
---@param config BasiliskConfig
function M.update(config)
  local current = binary.locate(config.binary_path)
  if not current then
    M.install(config)
    return
  end

  local advice = SOURCE_ADVICE[binary.install_source(current)]
  if advice then
    log.warn("%s", advice)
    return
  end

  local release = binary.fetch_latest_release()
  if not release then
    log.error("could not reach GitHub for the latest release — check your network")
    return
  end

  local current_version = binary.version(current)
  if current_version and not binary.is_newer_version(current_version, release.tag_name) then
    log.info("already up to date (%s)", current_version)
    return
  end

  confirm(
    string.format(
      "Update basilisk %s → %s (downloads from GitHub releases)?",
      current_version or "unknown version",
      release.tag_name
    ),
    "Update",
    function()
      download_and_restart(config)
    end
  )
end

--- :BasiliskInstall — first-use bootstrap when no binary is resolvable
--- ([NVIM-BINARY-UPGRADE-INSTALL]). Surfaces the auto-download that
--- resolve() step 7 performs, but announced and behind a confirmation.
---@param config BasiliskConfig
function M.install(config)
  local existing = binary.locate(config.binary_path)
  if existing then
    log.info(
      "basilisk already installed: %s (%s) — use :BasiliskUpdate to upgrade",
      existing,
      binary.version(existing) or "unknown version"
    )
    return
  end

  local release = binary.fetch_latest_release()
  if not release then
    log.error("could not reach GitHub for the latest release — check your network")
    return
  end

  local asset = binary.platform_asset_name()
  if not asset then
    log.error("no prebuilt binary for this platform — install with `cargo install basilisk-cli`")
    return
  end

  confirm(
    string.format("Install basilisk %s (downloads %s from GitHub releases)?", release.tag_name, asset),
    "Install",
    function()
      download_and_restart(config)
    end
  )
end

return M
