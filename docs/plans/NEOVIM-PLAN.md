# Basilisk Neovim Extension — Plan {#NVIMPLAN}

> **Spec**: `docs/specs/NEOVIM-SPEC.md` — read before touching any code.
> **Architecture model**: [rustaceanvim](https://github.com/mrcjkb/rustaceanvim)

---

## Status {#NVIMPLAN-STATUS}

The plugin **code** is largely complete (189 tests: 80 unit + 103 real LSP e2e + 6 screenshot regression, 0 failures; feature parity with VS Code/Zed for all LSP commands). The `Nimblesite/basilisk.nvim` mirror exists and is current.

**It is NOT yet fully live / usable out of the box.** Concrete gaps to close before it can be called shipped:

- [ ] **Register in nvim-lspconfig** — submit the `lsp/basilisk.lua` PR upstream so lspconfig users get Basilisk without manual `vim.lsp.config`. *(Phase 9; the single biggest "usable out of the box" blocker.)*
- [ ] **Real CI version matrix** — CI currently pins a single Neovim version (`v0.11.6` in `.github/workflows/ci.yml`); wire the actual `0.10 / 0.11 / nightly` matrix so we don't ship untested against the minimum supported and bleeding-edge versions. See [NVIM-DISTRIBUTION-CI].
- [ ] **Publish a tagged release on the mirror** — `Nimblesite/basilisk.nvim` has no GitHub Release (`latestRelease: null`); the `publish-nvim` job tags the mirror but no Release object exists for users who pin releases. Confirm the tag-sync actually fires on monorepo release and cut the first mirror release.
- [ ] **Verify outdated-binary warning + auto-download** end-to-end against real GitHub release assets (Phase 11 — implemented in `binary.lua` but unverified against published artifacts).
- [ ] **Strip committed coverage junk** — `basilisk.nvim/*.profraw` and `.pytest_cache/` should not live in the plugin tree that mirrors to users; gitignore + remove.

---

## Phase 1: Plugin Scaffolding & LSP Connection {#NVIMPLAN-SCAFFOLDING}

> Core structure, binary resolution, and LSP client lifecycle. The plugin must work with zero config after this phase.

## Phase 2: User Commands & Custom LSP Command Registration {#NVIMPLAN-USER-COMMANDS}

> Surface all LSP commands as `:Basilisk*` user commands with proper completion and error handling. Includes profiling, memory, and uv command modules.

## Phase 3: DAP Integration (nvim-dap) {#NVIMPLAN-DAP-INTEGRATION}

> Debug Adapter Protocol via nvim-dap. DapTcpProxy in Lua/libuv. Graceful degradation if nvim-dap is absent.

## Phase 4: Test Explorer {#NVIMPLAN-TEST-EXPLORER}

> Test discovery, tree UI, run/debug integration. See LSP-ARCHITECTURE-SPEC.md for supported frameworks.

## Phase 5: Keymaps & ftplugin {#NVIMPLAN-KEYMAPS}

> Default keymaps via `LspAttach` autocmd. All configurable, all disableable.

## Phase 6: Status Line {#NVIMPLAN-STATUS-LINE}

> Status line component compatible with lualine, heirline, and raw statusline.

## Phase 7: Health Check {#NVIMPLAN-HEALTH-CHECK}

> `:checkhealth basilisk` for diagnosing setup issues.

## Phase 8: Documentation & Help {#NVIMPLAN-DOCUMENTATION}

> Vim help file and inline documentation.

## Phase 9: CI & Distribution {#NVIMPLAN-CI-DISTRIBUTION}

> Automated testing, cross-version compatibility, and publishing.

## Phase 10: Automated UI Testing {#NVIMPLAN-UI-TESTING}

> End-to-end UI testing using `mini.test` (screenshot/snapshot testing) and headless Neovim (`nvim --embed` via RPC). Covers floating windows, extmarks, side panels, keymaps, and status line. Model: [mini.nvim test suite](https://github.com/echasnovski/mini.nvim).

---

## Rules {#NVIMPLAN-RULES}

- Plugin must work with **zero config** (`require('basilisk').setup({})`)
- All features degrade gracefully when optional dependencies are absent
- No hard dependency on nvim-lspconfig — use native `vim.lsp.config` + `vim.lsp.enable`
- nvim-dap is optional — all DAP features behind `pcall` guards
- All shared settings/commands defined in LSP-ARCHITECTURE-SPEC.md — never diverge
- Neovim 0.10 is the minimum supported version
- Follow [nvim-best-practices](https://github.com/nvim-neorocks/nvim-best-practices)
- Test with plenary.nvim — no unit test theatre, test real behavior

---

## TODO {#NVIMPLAN-TODO}

### Phase 1: Plugin Scaffolding & LSP Connection {#NVIMPLAN-TODO-SCAFFOLDING}

- [x] Create `basilisk.nvim/` directory with full plugin structure (`plugin/`, `lua/basilisk/`, `ftplugin/`, `after/lsp/`, `doc/`, `tests/`)
- [x] `plugin/basilisk.lua` — auto-loaded entry point with version guard (Neovim >= 0.10), user command registration, autocmds
- [x] `lua/basilisk/init.lua` — `setup()` entry, config merge with defaults, module orchestration
- [x] `lua/basilisk/config.lua` — default config table with LuaCATS type annotations, validation function
- [x] `lua/basilisk/log.lua` — logger wrapping `vim.notify` with configurable log levels + optional file logging
- [x] `lua/basilisk/binary.lua` — binary resolution cascade (user config → `BASILISK_PATH` env → `~/.cargo/bin/basilisk` → `/usr/local/bin/basilisk` → `/opt/homebrew/bin/basilisk` → PATH)
- [x] `lua/basilisk/lsp.lua` — `vim.lsp.config('basilisk', ...)` + `vim.lsp.enable('basilisk')` with all shared settings from LSP-ARCHITECTURE-SPEC.md
- [x] Pass all shared configuration settings to LSP server (`basilisk.python`, `basilisk.analysisMode`, `basilisk.inlayHints.*`, `basilisk.ruff.*`, `basilisk.debugger.*`, `basilisk.testExplorer.*`, `basilisk.uv.*`)
- [x] Error recovery — auto-restart up to 3 times with exponential backoff (1s, 2s, 4s), status tracking, `vim.notify` on max restarts
- [x] `after/lsp/basilisk.lua` — Neovim 0.11+ native LSP config fallback for non-`setup()` users
- [ ] Verify all 21 core LSP features work out of the box (requires running basilisk binary against a real Python project)

#### Tests (Phase 1) {#NVIMPLAN-TODO-SCAFFOLDING-TESTS}

- [x] `tests/minimal_init.lua` — isolated test init with plenary.nvim
- [x] `tests/basilisk/binary_spec.lua` — binary resolution cascade tests (5 tests passing)
- [x] `tests/basilisk/config_spec.lua` — config merge, validation, type annotation coverage (17 tests passing)
- [x] `tests/basilisk/lsp_spec.lua` — LSP client config generation, settings passthrough, error recovery logic (2 tests passing)

### Phase 2: User Commands & Custom LSP Command Registration {#NVIMPLAN-TODO-USER-COMMANDS}

- [x] `lua/basilisk/commands.lua` — command registration framework with `nvim_create_user_command`
- [x] `:BasiliskRestart` — client-side LSP restart with counter reset
- [x] `:BasiliskInfo` — floating window showing server status, binary path, Python interpreter, version info
- [x] `:BasiliskOrganizeImports` — send `basilisk.organizeImports` via `workspace/executeCommand`
- [x] Register `vim.lsp.commands['basilisk.organizeImports']` handler
- [x] `:BasiliskDebugFile` — send `basilisk/startDebugSession`, trigger DAP

#### Profiling Commands {#NVIMPLAN-TODO-USER-COMMANDS-PROFILING}

- [x] `lua/basilisk/profiling.lua` — profiling command module
- [x] `:BasiliskProfile [pid]` — send `basilisk/profiler/start` with optional PID argument
- [x] `:BasiliskProfileStop` — send `basilisk/profiler/stop`, display results in floating window + quickfix
- [x] `:BasiliskProfileSnapshot` — send `basilisk/profiler/snapshot`
- [x] Heat map UI — `nvim_buf_set_extmark()` with virtual text on hot lines
- [x] Flamegraph export — speedscope JSON + `vim.ui.open()` to browser
- [x] Hot function list — quickfix list or floating window

#### Memory Commands {#NVIMPLAN-TODO-USER-COMMANDS-MEMORY}

- [x] `lua/basilisk/memory.lua` — memory command module
- [x] `:BasiliskMemLeak` — send `basilisk/memory/start`
- [x] `:BasiliskMemStop` — send `basilisk/memory/stop`, display leak report in floating window
- [x] `:BasiliskMemRefs <Type>` — send `basilisk/memory/refs` with type argument, floating window with retention paths + confidence scores
- [x] Command completion for `:BasiliskMemRefs` — common types (DataFrame, dict, list, set, ndarray, Tensor) + workspace types

#### uv Commands {#NVIMPLAN-TODO-USER-COMMANDS-UV}

- [x] `:BasiliskUvSync` — send `basilisk.uv.sync`
- [x] `:BasiliskUvAdd <package>` — send `basilisk.uv.add` with completion
- [x] `:BasiliskUvAddDev <package>` — send `basilisk.uv.addDev` with completion
- [x] `:BasiliskUvRemove <package>` — send `basilisk.uv.remove` with completion
- [x] `:BasiliskUvLock` — send `basilisk.uv.lock`
- [x] `:BasiliskUvCreateEnv [version]` — send `basilisk.uv.createEnv`

### Phase 3: DAP Integration (nvim-dap) {#NVIMPLAN-TODO-DAP-INTEGRATION}

- [x] `lua/basilisk/dap.lua` — DAP module with `pcall(require, 'dap')` runtime detection
- [x] Register `dap.adapters.basilisk` — send `basilisk/startDebugSession` to LSP, receive `{host, port, sessionId}`, start DapTcpProxy, return server adapter
- [x] DapTcpProxy implementation using `vim.uv`:
  - [x] `vim.uv.new_tcp()` — TCP socket creation and management
  - [x] Content-Length header framing for DAP messages
  - [x] Intercept `stepOut` — inject auto-`next` for structural lines (try:, with:, if:)
  - [x] Inject `exited` event before `terminated` if missing
  - [x] Fast disconnect — respond immediately post-termination
  - [ ] Attach mode timeout — 3s timeout with synthetic success response (needs integration testing)
- [x] Default `dap.configurations.python` — launch (current file) + attach (127.0.0.1:5678)
- [x] `:BasiliskDebugFile` wired to DAP adapter
- [x] Optional nvim-dap-ui integration — auto-open on `event_initialized`, auto-close on `event_terminated`
- [x] Optional nvim-dap-virtual-text integration — enable for type-aware inline variable display
- [x] `basilisk/stopDebugSession` — send on session cleanup

#### Tests (Phase 3) {#NVIMPLAN-TODO-DAP-INTEGRATION-TESTS}

- [ ] DapTcpProxy integration tests — message framing, interception rules, timeout behavior (requires live TCP)
- [ ] DAP adapter registration test — verify adapter callback shape (requires nvim-dap)
- [x] Graceful degradation — no errors when nvim-dap is absent (verified via pcall guard)

### Phase 4: Test Explorer {#NVIMPLAN-TODO-TEST-EXPLORER}

> **DONE.** See `LSP-TEST-INTEGRATION-PLAN.md` for the cross-editor test integration plan and remaining TODO items.
> Neovim-specific tasks below are complete.

### Phase 5: Keymaps & ftplugin {#NVIMPLAN-TODO-KEYMAPS}

- [x] `ftplugin/python.lua` — auto-loaded for Python buffers
- [x] Standard LSP keymaps (buffer-local, set on `LspAttach`):
  - [x] `gd` → definition, `gD` → declaration, `gy` → type definition
  - [x] `gr` → references, `K` → hover, `<C-k>` → signature help
  - [x] `<leader>rn` → rename, `<leader>ca` → code action
- [x] Basilisk-specific keymaps (`<leader>b` prefix, configurable via `keymaps.prefix`):
  - [x] `<leader>br` → restart, `<leader>bo` → organize imports
  - [x] `<leader>bp` → start profiling, `<leader>bP` → stop profiling
  - [x] `<leader>bm` → start memory tracking, `<leader>bM` → stop memory tracking
  - [x] `<leader>bt` → toggle test explorer, `<leader>bd` → debug current file
  - [x] `<leader>bR` → run test at cursor, `<leader>bD` → debug test at cursor
- [x] Auto-enable inlay hints for Python buffers
- [x] Auto-enable code lens for Python buffers
- [x] `keymaps.enabled = false` disables all default keymaps

### Phase 6: Status Line {#NVIMPLAN-TODO-STATUS-LINE}

- [x] `lua/basilisk/statusline.lua` — status line module
- [x] Track LSP client state: starting / ready / error / stopped
- [x] Aggregate diagnostic counts (errors + warnings) from `vim.diagnostic.get()`
- [x] State display matching VS Code/Zed behavior:
  - [x] Starting → `⟳ Basilisk` (yellow)
  - [x] Ready (no errors) → `✓ Basilisk` (green)
  - [x] Ready (with errors) → `⚠ Basilisk (3E 2W)` (orange)
  - [x] Error → `✗ Basilisk` (red)
  - [x] Stopped → `⊘ Basilisk` (grey)
- [x] `lualine_component` export for drop-in lualine integration
- [x] Raw `statusline()` function for non-lualine users

### Phase 7: Health Check {#NVIMPLAN-TODO-HEALTH-CHECK}

- [x] `lua/basilisk/health.lua` — health check module using `vim.health`
- [x] Check Neovim version >= 0.10
- [x] Check `basilisk` binary found + report version
- [x] Check Python interpreter found + report version
- [x] Check `debugpy` installed (optional, for DAP)
- [x] Check `nvim-dap` available (optional, for debugging)
- [x] Check `nvim-dap-ui` available (optional, for debug UI)
- [x] Check `ruff` available (optional, for formatting)
- [x] Check `uv` available (optional, for package management)
- [x] Report configuration summary (analysis mode, enabled features)

### Phase 8: Documentation & Help {#NVIMPLAN-TODO-DOCUMENTATION}

- [x] `doc/basilisk.txt` — full Vim help file covering:
  - [x] Installation (lazy.nvim, packer, manual)
  - [x] Configuration reference (all settings with defaults)
  - [x] Commands reference (all `:Basilisk*` commands)
  - [x] Keymaps reference (all default keymaps)
  - [x] DAP setup guide
  - [x] Test explorer usage
  - [x] Profiling/memory workflow
  - [x] Troubleshooting (`:checkhealth`, common issues)
- [ ] Generate help tags (`helptags`) — done at install time by plugin managers

### Phase 9: CI & Distribution {#NVIMPLAN-TODO-CI-DISTRIBUTION}

- [ ] GitHub Actions CI — run plenary.nvim tests on the Neovim **0.10 / 0.11 / nightly** matrix. **Not done:** `ci.yml` currently pins a single version (`v0.11.6`); the matrix is not wired. See [NVIM-DISTRIBUTION-CI].
- [x] Test on macOS and Linux (via CI matrix)
- [x] lazy.nvim package spec (`{ 'Nimblesite/basilisk.nvim', ft = 'python' }`)
- [x] Release/tagging mechanism — `publish-nvim` job in `release.yml` subtree-splits
      `basilisk.nvim/` to the `Nimblesite/basilisk.nvim` mirror, tag-synced to monorepo releases
      (see `docs/plans/NEOVIM-RELEASE-PLAN.md`; `[HUMAN]` gap: grant the existing `BREW_SCOOP_PAT`
      write access to the `Nimblesite/basilisk.nvim` mirror)
- [ ] Submit `lsp/basilisk.lua` PR to nvim-lspconfig for basic LSP support

> Version check (warn on outdated binary) and binary auto-download are tracked once, under Phase 11 "Missing Features".

### Phase 10: Automated UI Testing {#NVIMPLAN-TODO-UI-TESTING}

> Framework: **plenary.nvim** for headless testing + **headless Neovim** (`nvim --headless`) via API assertions. Tests cover status line, test tree parsing, memory completion, and module behavior.

#### Test Infrastructure {#NVIMPLAN-TODO-UI-TESTING-INFRASTRUCTURE}

- [x] `tests/ui/helpers.lua` — shared utilities: wait for condition, find floating window, assert extmarks, buffer keymaps
- [ ] CI integration — run UI tests in GitHub Actions via `nvim --headless` on the Neovim **0.10 / 0.11 / nightly** matrix (currently single-version only; see [NVIM-DISTRIBUTION-CI])

#### Status Line Tests (10 tests passing) {#NVIMPLAN-TODO-UI-TESTING-STATUS-LINE}

- [x] `require('basilisk.statusline').get()` — assert correct string for each state (starting, ready, error, stopped)
- [x] `lualine_component` — assert it returns a valid lualine component table with callable function and color
- [x] State transitions — simulate state changes and assert status/color updates

#### Test Tree Tests (11 tests passing) {#NVIMPLAN-TODO-UI-TESTING-TEST-TREE}

- [x] Parse simple test output — file > function hierarchy
- [x] Parse class-based tests — file > class > function hierarchy
- [x] Handle multiple files
- [x] Skip empty/summary lines
- [x] Preserve full test IDs
- [x] Default status to unknown
- [x] Memory completion — returns matching types, case-insensitive

#### Live LSP UI Tests (in `tests/lsp/ui_spec.lua`) {#NVIMPLAN-TODO-UI-TESTING-LIVE-LSP}

- [x] `:BasiliskInfo` — assert floating window opens with correct content (title, status, binary, version, mode)
- [x] `:BasiliskTestToggle` — assert side panel opens with correct filetype/width, toggle closes it
- [x] Screenshot / snapshot tests with mini.test — 6 tests covering diagnostics, info float, test panel, diagnostic float, statusline
- [x] Reference screenshots stored in `tests/ui/screenshots/` — 6 reference files for regression comparison

---

## Phase 11: Feature Parity & E2E Test Coverage Gaps {#NVIMPLAN-FEATURE-PARITY}

> Close all feature parity gaps with VS Code/Zed extensions and ensure every command has a real LSP e2e test.

### Missing Features (implement + test) {#NVIMPLAN-FEATURE-PARITY-MISSING-FEATURES}

- [x] `:BasiliskDisableRule <code>` — send `basilisk.disableRule` via LSP (VS Code/Zed have this)
- [ ] Version check — warn user if basilisk binary is outdated (Zed has this)
- [ ] Binary auto-download fallback from GitHub releases (Zed has this)

### Commands — Real LSP E2E Tests {#NVIMPLAN-FEATURE-PARITY-E-TO-E-TESTS}

#### Workspace Commands (in `commands_spec.lua`) {#NVIMPLAN-FEATURE-PARITY-E-TO-E-TESTS-WORKSPACE}

- [x] `:BasiliskFixWorkspace` — send LSP command, verify no error
- [x] `:BasiliskAdoptWorkspace` — send LSP command, verify no error
- [x] `:BasiliskUnadoptFile` — send LSP command, verify no error
- [x] `:BasiliskDisableRule BSK-E0001` — send LSP command, verify pyproject.toml modified
- [x] `:BasiliskShowOutput` — verify log buffer opens

#### uv Commands (in `uv_spec.lua`) {#NVIMPLAN-FEATURE-PARITY-E-TO-E-TESTS-UV}

- [x] `:BasiliskUvSync` — send real LSP command, verify no error
- [x] `:BasiliskUvAdd <pkg>` — send real LSP command, verify no error
- [x] `:BasiliskUvAddDev <pkg>` — send real LSP command, verify no error
- [x] `:BasiliskUvRemove <pkg>` — send real LSP command, verify no error
- [x] `:BasiliskUvLock` — send real LSP command, verify no error
- [x] `:BasiliskUvCreateEnv` — send real LSP command, verify no error

#### Profiling Commands (in `commands_spec.lua`) {#NVIMPLAN-FEATURE-PARITY-E-TO-E-TESTS-PROFILING}

- [x] `:BasiliskProfile` — send `basilisk/profiler/start` to real server
- [x] `:BasiliskProfileStop` — send `basilisk/profiler/stop` to real server
- [x] `:BasiliskProfileSnapshot` — send `basilisk/profiler/snapshot` to real server

#### Memory Commands (in `commands_spec.lua`) {#NVIMPLAN-FEATURE-PARITY-E-TO-E-TESTS-MEMORY}

- [x] `:BasiliskMemLeak` — send `basilisk/memory/start` to real server
- [x] `:BasiliskMemStop` — send `basilisk/memory/stop` to real server
- [x] `:BasiliskMemRefs <type>` — send `basilisk/memory/refs` to real server

#### Tab Tracking (in `analysis_mode_spec.lua`) {#NVIMPLAN-FEATURE-PARITY-E-TO-E-TESTS-TAB-TRACKING}

- [x] Open file → wipeout buffer → verify diagnostics cleared
- [x] Reopen closed file → verify diagnostics re-triggered

#### Refactoring Commands (in `commands_spec.lua`) {#NVIMPLAN-FEATURE-PARITY-E-TO-E-TESTS-REFACTORING}

- [x] `:BasiliskExtractVariable` — trigger code action on selection
- [x] `:BasiliskExtractConstant` — trigger code action on selection
- [x] `:BasiliskConvertUnion` — trigger code action
- [x] `:BasiliskImplementMethods` — trigger code action
