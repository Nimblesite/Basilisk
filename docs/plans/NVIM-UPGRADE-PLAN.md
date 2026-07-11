# basilisk.nvim — Binary Upgrade Story Plan `[NVIM-UPGRADE]`

> Spec: [NVIM-BINARY-UPGRADE] in [NEOVIM-SPEC.md](../specs/NEOVIM-SPEC.md).
> Status: core deliverables (1, 2, 3, docs half of 4, 5) **shipped**; the
> Mason registry submission and release-asset follow-ups remain open.

## Problem

The `basilisk.nvim` plugin detected a newer release and printed
`[basilisk] update available: <cur> → v0.33.0. Run :checkhealth basilisk for details.`
([`binary.lua` `check_for_updates`](../../basilisk.nvim/lua/basilisk/binary.lua)).

That notice was a **dead end**: `:checkhealth basilisk` only shows a status list, and
there was **no in-editor action to actually install the new version**. Users had no
"normal way" to upgrade the binary from nvim.

Worse, the download engine itself was broken on macOS: `platform_asset_name()`
asked for `basilisk-aarch64-apple-darwin.tar.gz` but `release.yml` publishes a
`.zip` whose binaries are nested under a `basilisk-darwin/` staging dir — the
downloader silently found no asset ([NVIM-BINARY-UPGRADE-ASSETS]).

## What already existed (built on it, not duplicated) — Rule: reduce duplication

- `M.download()` — full working downloader: `platform_asset_name()` →
  `fetch_latest_release()` → curl → tar/unzip → chmod. Downloads
  `releases/latest`, previously called only as fallback step 7 of `resolve()`.
- `is_newer_version()` / `parse_semver()` — version comparison, unit-tested.
- `check_for_updates()` — the notifier that produces the message on screen.
- The `:Basilisk*` user-command surface in `commands.lua`.

**The engine was done. The gap was a user-facing action + distribution surfaces + docs.**

## Deliverables — "all the things people normally do"

### 1. In-editor self-update — `:BasiliskUpdate` ✅ SHIPPED
- `:BasiliskUpdate` in `commands.lua`, delegating to
  [`update.lua`](../../basilisk.nvim/lua/basilisk/update.lua), which reuses
  `download()` (no copy-paste of curl/extract logic).
- Downloads into the versioned cache dir (`stdpath("data")/basilisk/<tag>/`),
  sets `binary_path`, force-restarts the LSP client.
- Confirmation UX: `vim.ui.select({"Update now","Later"})` — the real "accept"
  step in the TUI ([NVIM-BINARY-UPGRADE-CONFIRM]).
- Refuses gracefully when the resolved binary is a **local dev build**
  (`0.0.0-PLACEHOLDER`) or a package-manager install (Homebrew/Scoop/cargo):
  tells the user the owning upgrade command instead of clobbering it
  ([NVIM-BINARY-UPGRADE-SOURCES]).
- The notification is actionable: `check_for_updates` names the owning
  upgrade action (`:BasiliskUpdate`, `brew upgrade basilisk`, …) — never
  `:checkhealth` ([NVIM-BINARY-UPGRADE-NOTICE]).

### 2. `:BasiliskInstall` / bootstrap ✅ SHIPPED
- Installs the binary on first use when nothing is resolvable, surfacing the
  auto-download that `resolve()` step 7 already performed but never announced
  ([NVIM-BINARY-UPGRADE-INSTALL]). `:checkhealth basilisk` advice now names it.

### 3. Plugin-manager guidance (how the *Lua* half updates) ✅ SHIPPED
- README (en + zh): copy-paste install specs for lazy.nvim, packer, vim-plug,
  vim.pack, plus an Updating section (`:Lazy update` / `:PackerSync` /
  `:PlugUpdate` for the plugin; `:BasiliskUpdate` for the binary).
- Same story in `doc/basilisk.txt` (`:h basilisk-binary`) and the website
  guide [`/docs/install-neovim/`](../../website/src/docs/install-neovim.md).

### 4. Distribution surfaces for the *binary* (what CI must publish)
- ✅ **Asset naming verified & fixed client-side**: `platform_asset_name()` now
  byte-matches the five archives `release.yml` publishes (Linux `.tar.gz`,
  macOS/Windows `.zip`); zip extraction flattens the macOS staging dir and
  chmods `basilisk-profiler-helper`. A binary_spec contract test pins the
  exact published names ([NVIM-BINARY-UPGRADE-ASSETS]).
- ✅ **Homebrew / Scoop**: `publish-homebrew` / `publish-scoop` jobs in
  `release.yml` push the tap/bucket on every tag; the update notice shows
  `brew upgrade basilisk` / `scoop update basilisk` for those installs.
- ⬜ **Mason**: submit `basilisk` to the upstream
  [mason-registry](https://github.com/mason-org/mason-registry) so
  `:MasonInstall basilisk` / `:MasonUpdate` work. External PR — the release
  assets it needs already exist. Do not document Mason support until the
  registry entry is merged.
- ⬜ **macOS x86_64**: no `x86_64-apple-darwin` release asset is built;
  `platform_asset_name()` deliberately returns `nil` there and the flows
  advise `cargo install basilisk-cli`. Add the build to `release.yml` if
  Intel-mac demand appears.

### 5. Tests (never fewer failing tests; ratchet up) ✅ SHIPPED
- `binary_spec.lua`: published-asset-name contract, per-OS asset assertions,
  `install_source` classification, `upgrade_hint` mapping, actionable-notice
  regression (must name `:BasiliskUpdate`, must not name `:checkhealth`),
  dev-build silence.
- `update_spec.lua` (new): happy path, decline no-op, already-latest no-op,
  Homebrew/cargo/dev refusals, unreachable-GitHub error, install bootstrap,
  install-when-present redirect, download-failure report.
- `commands_spec.lua`: `:BasiliskUpdate` / `:BasiliskInstall` registration.

## Explicitly NOT touched
- The Rust checker / conformance harness — this is plugin + distribution only.
- No conformance ratchet, rule, or `basilisk.json` change (forbidden per CLAUDE.md).
