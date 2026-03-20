# uv Integration Plan

> **Spec**: `docs/specs/LSP-UV-INTEGRATION-SPEC.md`
> **LSP Spec**: `docs/specs/LSP-ARCHITECTURE-SPEC.md` — commands, config, file watchers
> **Depends on**: Import resolver (working), config system (working), file watchers (working)
> **Branch**: `uv-integration`

---

## Current State

The LSP has working import resolution with PEP 561 search order, venv detection (`.venv`, `venv`, `.env`, `env`), site-packages resolution, and workspace scanning. But it has **zero uv awareness** — no lock file parsing, no workspace detection, no uv-specific diagnostics, no package intelligence.

### What Exists

| Component | File | Status |
|-----------|------|--------|
| Venv detection | `crates/basilisk-lsp/src/import_resolver.rs` | Working — filesystem probe only |
| Site-packages resolution | `crates/basilisk-lsp/src/import_resolver.rs` | Working — from detected venv |
| Config loading | `crates/basilisk-lsp/src/config.rs` | Working — basilisk.json, pyproject.toml, pyrightconfig.json |
| File watchers | `crates/basilisk-lsp/src/server.rs` | Working — `.py`/`.pyi` files |
| BSK-E0010 (unresolved import) | `crates/basilisk-checker/src/rules/e0010.rs` | Working — generic message |
| Stub recognition | `crates/basilisk-stubs/src/lib.rs` | Working — stdlib via `phf`, builtins |
| TOML parsing | `Cargo.toml` dependencies | `toml` crate already used for config |

### What's Missing

- No `uv.lock` parser
- No uv project detection
- No `PackageRegistry`
- No uv workspace discovery
- No `.python-version` reading
- No uv-aware diagnostics or code actions
- No uv commands
- No package-to-import-name mapping

---

## Phase 1: uv Project Detection & Lock File Parsing

> **Goal**: Detect uv projects and parse `uv.lock` into a `PackageRegistry`. Zero subprocess calls.

### 1.1 New Crate: `basilisk-uv`

Create a dedicated crate for all uv integration logic. Keeps the LSP crate focused and enables unit testing of lock file parsing independently.

| Task | File | Description |
|------|------|-------------|
| Create crate | `crates/basilisk-uv/Cargo.toml` | Dependencies: `toml`, `serde`, `tracing`, `thiserror` |
| uv project detection | `crates/basilisk-uv/src/detect.rs` | `detect_uv_project(workspace_roots) -> Option<UvProjectInfo>` — check `uv.lock`, `pyproject.toml [tool.uv]`, `.venv/pyvenv.cfg` |
| Lock file parser | `crates/basilisk-uv/src/lockfile.rs` | `parse_lock_file(path) -> Result<LockFile>` — deserialize `uv.lock` TOML into structured data |
| Package registry | `crates/basilisk-uv/src/registry.rs` | `PackageRegistry` — HashMap lookup by normalized import name, direct/dev/transitive classification |
| Import name mapping | `crates/basilisk-uv/src/import_map.rs` | `package_to_import_name(package_name) -> String` — known mismatches table + normalization fallback |
| Python version reader | `crates/basilisk-uv/src/python_version.rs` | `read_python_version(root) -> Option<String>` — parse `.python-version` file |
| Crate lib | `crates/basilisk-uv/src/lib.rs` | Public API re-exports |

### 1.2 Lock File Data Model

```rust
// Deserialized from uv.lock TOML
#[derive(Deserialize)]
pub struct LockFile {
    pub version: u32,
    #[serde(rename = "requires-python")]
    pub requires_python: Option<String>,
    #[serde(rename = "package", default)]
    pub packages: Vec<LockPackage>,
}

#[derive(Deserialize)]
pub struct LockPackage {
    pub name: String,
    pub version: String,
    pub source: Option<LockSource>,
    #[serde(default)]
    pub dependencies: Vec<LockDependency>,
}
```

### 1.3 Tests

| Test | Description |
|------|-------------|
| `detect_uv_project_with_lockfile` | Directory with `uv.lock` → detected |
| `detect_uv_project_with_tool_uv` | `pyproject.toml` with `[tool.uv]` only → detected |
| `detect_uv_project_venv_marker` | `.venv/pyvenv.cfg` with `uv = true` → detected |
| `detect_non_uv_project` | Poetry project with `poetry.lock` → not detected |
| `parse_lockfile_basic` | Parse a minimal `uv.lock` fixture → correct packages |
| `parse_lockfile_with_markers` | Parse lock with platform markers → filtered correctly |
| `parse_lockfile_workspace` | Parse lock with workspace members → members identified |
| `package_registry_lookup` | Registry lookup by import name → correct PackageInfo |
| `import_name_mapping` | `Pillow` → `PIL`, `scikit-learn` → `sklearn`, `python-dateutil` → `dateutil` |
| `read_python_version` | `.python-version` containing `3.12` → `Some("3.12")` |
| `read_python_version_missing` | No `.python-version` → `None` |

**Deliverable**: `cargo test -p basilisk-uv` passes. Lock file parsing works on real-world `uv.lock` files.

---

## Phase 2: Wire into Import Resolver

> **Goal**: The import resolver uses `PackageRegistry` to validate imports and provide richer resolution metadata.

### 2.1 Import Resolver Integration

| Task | File | Description |
|------|------|-------------|
| Add `PackageRegistry` to search paths | `crates/basilisk-lsp/src/import_resolver.rs` | `ImportSearchPaths` gains `registry: Option<Arc<PackageRegistry>>` |
| Validate imports against registry | `crates/basilisk-lsp/src/import_resolver.rs` | After filesystem resolution, cross-check with registry for metadata |
| Classify unresolved imports | `crates/basilisk-lsp/src/import_resolver.rs` | `UnresolvedReason` enum: `NotInstalled`, `NotInDeps`, `NeedsSync`, `NoStubs`, `WrongPythonVersion` |
| Pass registry to workspace | `crates/basilisk-lsp/src/workspace.rs` | `WorkspaceIndex` holds `Arc<PackageRegistry>`, passed to resolver |

### 2.2 Extended Resolution Result

```rust
pub struct ResolvedImport {
    pub path: PathBuf,
    pub resolution: ImportResolution,
    pub package_info: Option<Arc<PackageInfo>>, // NEW: from registry
}

pub enum UnresolvedReason {
    NotInstalled,       // not in uv.lock at all
    NotInDeps,          // in lock as transitive, not in pyproject.toml
    NeedsSync,          // in pyproject.toml but lock is stale
    NoStubs,            // installed but no .pyi files
    WrongPythonVersion, // stdlib module not available in target version
    Unknown,            // non-uv project, can't determine
}
```

### 2.3 Initialization Flow

On LSP startup:

1. `WorkspaceConfig` loads from config files (existing)
2. `detect_uv_project()` checks workspace roots (new)
3. If uv detected: parse `uv.lock` → build `PackageRegistry` (new)
4. `ImportSearchPaths` constructed with registry (extended)
5. Workspace scan proceeds as normal

### 2.4 Tests

| Test | Description |
|------|-------------|
| `resolve_import_with_registry` | Import `requests` in uv project → resolved with `PackageInfo` attached |
| `unresolved_not_installed` | Import `nonexistent` → `UnresolvedReason::NotInstalled` |
| `unresolved_needs_sync` | Package in `pyproject.toml` but not in `.venv` → `NeedsSync` |
| `resolve_workspace_member` | Import workspace member `my_lib` → resolved as editable |
| `fallback_without_uv` | Non-uv project → existing resolution unchanged |

**Deliverable**: Import resolution is enriched with package metadata in uv projects. Non-uv projects behave identically to before.

---

## Phase 3: Enhanced Diagnostics

> **Goal**: BSK-E0010 becomes actionable. New stub suggestion diagnostic. Dependency hygiene warnings.

### 3.1 Actionable BSK-E0010

| Task | File | Description |
|------|------|-------------|
| Pass `UnresolvedReason` to checker | `crates/basilisk-checker/src/rules/e0010.rs` | E0010 diagnostic message varies by reason |
| Context-aware messages | `crates/basilisk-checker/src/rules/e0010.rs` | See spec section 5.1 for message table |
| Add diagnostic data | `crates/basilisk-checker/src/rules/e0010.rs` | Attach `code_action_data` to diagnostic for code action provider |

### 3.2 New: BSK-W0010 (Missing Stubs)

| Task | File | Description |
|------|------|-------------|
| New rule | `crates/basilisk-checker/src/rules/w0010.rs` | Fires when package is installed but has no type info |
| Stub package lookup | `crates/basilisk-uv/src/registry.rs` | `find_stub_package(name) -> Option<String>` — checks `types-{name}` and `{name}-stubs` patterns |
| Wire into checker | `crates/basilisk-checker/src/lib.rs` | Register W0010, pass `PackageRegistry` to check cycle |

### 3.3 New: Dependency Hygiene (BSK-W0011, W0012, W0013)

| Task | File | Description |
|------|------|-------------|
| W0011: undeclared dep import | `crates/basilisk-checker/src/rules/w0011.rs` | Import of package not in `[project.dependencies]` |
| W0012: unused dep | `crates/basilisk-checker/src/rules/w0012.rs` | Package in deps but never imported (whole-module mode only) |
| W0013: stale lock | `crates/basilisk-checker/src/rules/w0013.rs` | `pyproject.toml` mtime > `uv.lock` mtime |
| Gate behind config | `crates/basilisk-lsp/src/config.rs` | Only active when `basilisk.uv.dependencyDiagnostics = true` |

### 3.4 Tests

| Test | Description |
|------|-------------|
| `e0010_not_installed_message` | Import of uninstalled package → message includes "not a dependency" |
| `e0010_needs_sync_message` | In pyproject.toml but not synced → message includes "uv sync" |
| `w0010_missing_stubs` | Import `requests` (installed, no stubs) → W0010 fires with stub suggestion |
| `w0010_stubs_installed` | Import `requests` (stubs installed) → W0010 does NOT fire |
| `w0011_undeclared_import` | Import package not in pyproject.toml → W0011 fires |
| `w0013_stale_lock` | pyproject.toml newer than uv.lock → W0013 fires |
| `non_uv_project_no_new_diagnostics` | Non-uv project → none of W0010/W0011/W0012/W0013 fire |

**Deliverable**: Diagnostics are actionable and context-aware in uv projects. Zero behavior change for non-uv projects.

---

## Phase 4: Code Actions & LSP Commands

> **Goal**: One-click fixes for missing deps, missing stubs, stale locks. LSP commands for uv operations.

### 4.1 Code Actions

| Task | File | Description |
|------|------|-------------|
| "Add dependency" action | `crates/basilisk-lsp/src/code_actions.rs` | On BSK-E0010 (NotInstalled) → `basilisk.uv.add` command |
| "Install type stubs" action | `crates/basilisk-lsp/src/code_actions.rs` | On BSK-W0010 → `basilisk.uv.addDev` command |
| "Sync environment" action | `crates/basilisk-lsp/src/code_actions.rs` | On BSK-W0013 → `basilisk.uv.sync` command |

### 4.2 LSP Command Handlers

| Task | File | Description |
|------|------|-------------|
| uv command executor | `crates/basilisk-lsp/src/uv_commands.rs` | Subprocess runner — spawns `uv` with args, streams output, handles errors |
| `basilisk.uv.sync` | `crates/basilisk-lsp/src/uv_commands.rs` | Run `uv sync` in project root |
| `basilisk.uv.add` | `crates/basilisk-lsp/src/uv_commands.rs` | Run `uv add <package>` |
| `basilisk.uv.addDev` | `crates/basilisk-lsp/src/uv_commands.rs` | Run `uv add --dev <package>` |
| `basilisk.uv.remove` | `crates/basilisk-lsp/src/uv_commands.rs` | Run `uv remove <package>` |
| `basilisk.uv.lock` | `crates/basilisk-lsp/src/uv_commands.rs` | Run `uv lock` |
| `basilisk.uv.createEnv` | `crates/basilisk-lsp/src/uv_commands.rs` | Run `uv venv [--python X.Y]` |
| Register commands | `crates/basilisk-lsp/src/server.rs` | Register all `basilisk.uv.*` commands |

### 4.3 Post-Command Hook

Every successful `uv` command triggers:

1. Re-parse `uv.lock` (if it exists after command)
2. Rebuild `PackageRegistry`
3. Re-resolve imports for affected files
4. Publish updated diagnostics

### 4.4 Tests

| Test | Description |
|------|-------------|
| `code_action_add_dep` | E0010 on uninstalled package → code action with correct command |
| `code_action_add_stubs` | W0010 on stubless package → code action with `--dev` flag |
| `code_action_sync` | W0013 stale lock → code action to sync |
| `uv_command_sync` | Execute `basilisk.uv.sync` → subprocess runs, lock re-parsed |
| `uv_command_add` | Execute `basilisk.uv.add {package: "flask"}` → subprocess runs with correct args |
| `uv_binary_not_found` | uv not on PATH → graceful error, code actions hidden |

**Deliverable**: Users can fix dependency issues with one click directly from diagnostics.

---

## Phase 5: File Watchers & Hot Reload

> **Goal**: Environment stays perfectly in sync without LSP restarts.

### 5.1 Additional File Watchers

| Task | File | Description |
|------|------|-------------|
| Register `uv.lock` watcher | `crates/basilisk-lsp/src/server.rs` | `workspace/didChangeWatchedFiles` registration |
| Register `.python-version` watcher | `crates/basilisk-lsp/src/server.rs` | Watch for Python version changes |
| Handle `uv.lock` change | `crates/basilisk-lsp/src/server.rs` | Trigger lock re-parse → registry rebuild → diagnostic refresh |
| Handle `.python-version` change | `crates/basilisk-lsp/src/server.rs` | Trigger Python version update → stdlib availability recheck |
| Handle `pyproject.toml` change | `crates/basilisk-lsp/src/server.rs` | Detect workspace member changes, lock staleness |

### 5.2 Incremental Registry Update

When `uv.lock` changes:

```
1. Parse new lock file
2. Diff: added = new.packages - old.packages
         removed = old.packages - new.packages
3. For removed packages: find files importing them → mark for re-check
4. For added packages: find files with E0010 for those names → mark for re-check
5. Swap registry (Arc::swap)
6. Re-check marked files
7. Publish diagnostics
```

### 5.3 Tests

| Test | Description |
|------|-------------|
| `lockfile_change_triggers_reparse` | Modify `uv.lock` → registry updated |
| `lockfile_add_package_clears_e0010` | Add package to lock → E0010 for that import disappears |
| `lockfile_remove_package_fires_e0010` | Remove package from lock → E0010 appears |
| `python_version_change` | Change `.python-version` from 3.11 to 3.12 → stdlib diagnostics updated |

**Deliverable**: The LSP seamlessly tracks environment changes. Zero restarts needed.

---

## Phase 6: Hover Enrichment & Workspace Support

> **Goal**: Rich hover metadata for imports. Correct resolution in uv workspaces.

### 6.1 Hover Enrichment

| Task | File | Description |
|------|------|-------------|
| Package info in hover | `crates/basilisk-lsp/src/hover.rs` | When hovering import, append version + source + stub status |
| Workspace member hover | `crates/basilisk-lsp/src/hover.rs` | Show "workspace member" + path for workspace imports |

### 6.2 uv Workspace Resolution

| Task | File | Description |
|------|------|-------------|
| Parse `[tool.uv.workspace]` | `crates/basilisk-uv/src/workspace.rs` | Extract member paths from glob patterns |
| Workspace member discovery | `crates/basilisk-uv/src/workspace.rs` | Expand globs → enumerate member `pyproject.toml` files → extract package names |
| Wire into import resolver | `crates/basilisk-lsp/src/import_resolver.rs` | Workspace members added as import search paths (before site-packages) |
| Multi-root LSP mapping | `crates/basilisk-lsp/src/workspace.rs` | Map workspace members to LSP workspace folders |

### 6.3 Tests

| Test | Description |
|------|-------------|
| `hover_shows_package_version` | Hover on `import requests` → popup includes "v2.31.0" |
| `hover_shows_stub_status` | Hover on `import flask` → popup shows stub availability |
| `hover_workspace_member` | Hover on workspace member import → shows "workspace member" |
| `workspace_member_resolves` | Import of workspace member → resolves to member source root |
| `workspace_glob_expansion` | `members = ["packages/*"]` → discovers all members |
| `workspace_cross_member_no_e0010` | Import between workspace members → no E0010 |

**Deliverable**: Hover popups provide real context. Workspace monorepos just work.

---

## Phase 7: Configuration & Editor Integration

> **Goal**: uv settings in config, editor extensions updated.

### 7.1 Configuration

| Task | File | Description |
|------|------|-------------|
| Add `uv` config section | `crates/basilisk-lsp/src/config.rs` | `UvConfig` struct with fields from spec section 10 |
| Read from `basilisk.json` | `crates/basilisk-lsp/src/config.rs` | Deserialize `uv` key |
| Read from `pyproject.toml` | `crates/basilisk-lsp/src/config.rs` | `[tool.basilisk.uv]` section |
| uv binary resolution | `crates/basilisk-uv/src/binary.rs` | Resolution cascade from spec section 10.1 |
| Update LSP-ARCHITECTURE-SPEC.md | `docs/specs/LSP-ARCHITECTURE-SPEC.md` | Add uv settings to shared configuration table |

### 7.2 Editor Extension Updates

| Task | File | Description |
|------|------|-------------|
| VS Code settings | VS Code extension `package.json` | Add `basilisk.uv.*` settings |
| Neovim config docs | `docs/specs/NEOVIM-SPEC.md` | Document uv settings |
| Zed config docs | `docs/specs/ZED-SPEC.md` | Document uv settings |

### 7.3 Tests

| Test | Description |
|------|-------------|
| `config_uv_enabled_default` | Default config → `uv.enabled = true` |
| `config_uv_disabled` | `basilisk.json` with `uv.enabled = false` → no uv detection |
| `config_uv_executable_path` | Explicit path → used instead of auto-detection |
| `uv_binary_resolution_cascade` | Test each step of binary resolution |

**Deliverable**: Full configurability. All editors support uv settings.

---

## Execution Order & Dependencies

```
Phase 1 ─────────────────────────────┐
  (basilisk-uv crate, lock parsing)  │
                                      ├──► Phase 2 ──► Phase 3 ──► Phase 4
Phase 5 can start after Phase 2       │    (resolver)  (diags)     (actions)
                                      │
Phase 6 can start after Phase 2  ─────┘

Phase 7 can start after Phase 1 (config is independent)

Parallelizable:
  - Phase 5 + Phase 6 (independent after Phase 2)
  - Phase 7 (independent after Phase 1)
```

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| `uv.lock` format changes | Pin to lock file `version` field. Log warning on unknown version, fall back to filesystem-only |
| Large lock files (1000+ packages) | Parse is still fast (TOML in Rust). Registry is a HashMap — O(1) lookup |
| Package name ≠ import name | Known mapping table covers top 200 packages. Filesystem fallback for unknown |
| uv not installed | Lock file parsing still works (it's just a file). Only `basilisk.uv.*` commands disabled |
| Non-uv projects regress | All uv code paths gated behind `detect_uv_project()`. Comprehensive non-uv test suite |
| Subprocess hangs | 30-second timeout on all `uv` subprocess calls. Kill on timeout, report error |

---

## Success Metrics

| Metric | Target |
|--------|--------|
| uv project detection | < 1ms (filesystem stat only) |
| Lock file parse (100 packages) | < 5ms |
| Lock file parse (1000 packages) | < 50ms |
| Registry lookup | < 1 microsecond (HashMap) |
| Hot reload on `uv.lock` change | < 100ms to updated diagnostics |
| Zero regressions | All existing tests pass, non-uv projects unchanged |

---

## Todo

> **Philosophy**: We are not rebuilding uv. We read its files, call its CLI, and surface its intelligence inside the LSP. If uv already does it (env creation, dependency resolution, lock management), we just invoke `uv`. Our value-add is the _glue_: knowing what uv knows so we can give developers instant, in-editor feedback without them ever leaving their code.

### Phase 1 — uv Project Detection & Lock File Parsing

- [x] Create `crates/basilisk-uv` crate (`Cargo.toml`, `src/lib.rs`)
- [x] `detect.rs` — detect uv projects via `uv.lock`, `[tool.uv]`, `.venv/pyvenv.cfg`
- [x] `lockfile.rs` — parse `uv.lock` TOML into `LockFile` / `LockPackage` structs
- [x] `registry.rs` — `PackageRegistry` HashMap: normalized import name → `PackageInfo`
- [x] `import_map.rs` — package-to-import-name mapping (top 200 mismatches + normalization fallback)
- [x] `python_version.rs` — read `.python-version` file
- [x] Tests: detection (uv vs non-uv), lock parsing (basic, markers, workspace), registry lookup, import name mapping, `.python-version` — **67 tests passing**

### Phase 2 — Wire into Import Resolver

- [x] Add `Option<Arc<PackageRegistry>>` to `ImportSearchPaths`
- [x] Cross-check resolved imports against registry for metadata enrichment
- [x] `UnresolvedReason` enum (`NotInstalled`, `NotInDeps`, `NeedsSync`, `NoStubs`, `WrongPythonVersion`, `Unknown`)
- [x] `WorkspaceIndex` holds `Option<Arc<PackageRegistry>>` field
- [x] Startup flow: detect uv → parse lock → extract pyproject deps → build registry → pass to resolver
- [x] `PackageDepKind` enum on `ImportInfo` — set during `resolve_workspace_imports` from registry
- [x] `pyproject.rs` — extract `[project].dependencies` from `pyproject.toml` (PEP 508 specifier parsing)
- [x] Tests: resolution with registry, unresolved classification, workspace member resolution, non-uv fallback — **77 tests passing**

### Phase 3 — Enhanced Diagnostics

- [x] BSK-E0010: context-aware messages based on `UnresolvedReason` (not just "unresolved import")
- [x] BSK-E0010: attach `code_action_data` to diagnostic for quick-fix wiring
- [x] BSK-W0010: missing stubs diagnostic (package installed but no `.pyi`)
- [x] BSK-W0011: undeclared dependency import (transitive dep used directly) — fires when `package_dep_kind == Transitive`
- [x] BSK-W0012: unused dependency (in deps but never imported — whole-module only) — skeleton ready, awaits workspace-level aggregate import data
- [x] BSK-W0013: stale lock (`pyproject.toml` mtime > `uv.lock` mtime) — skeleton ready
- [x] Gate W0010 behind `uv.stubSuggestions` config (default true)
- [x] Gate W0011–W0013 behind `uv.dependencyDiagnostics` config (default false)
- [x] Config parsing: `basilisk.json` `uv.stubSuggestions`/`uv.dependencyDiagnostics`, `pyproject.toml` `[tool.basilisk.uv]`
- [x] Tests: message variants, stub detection, non-uv projects unchanged

### Phase 4 — Code Actions & LSP Commands (delegate to `uv` CLI)

- [x] Code action: "Add dependency" on E0010 (NotInstalled) → `uv add <package>`
- [x] Code action: "Install type stubs" on W0010 → `uv add --dev <stubs-package>`
- [x] Code action: "Sync environment" on W0013 → `uv sync`
- [x] `uv_commands.rs` — thin subprocess wrapper: spawn `uv` with args, 30s timeout, stream output
- [x] LSP commands: `basilisk.uv.sync`, `basilisk.uv.add`, `basilisk.uv.addDev`, `basilisk.uv.remove`, `basilisk.uv.lock`, `basilisk.uv.createEnv`
- [x] Post-command hook: `run_uv_and_refresh()` — all uv commands trigger `rebuild_registry_and_resolve()` on success
- [x] Graceful degradation: hide uv commands/actions when `uv` binary not found
- [x] Tests: code action generation, command execution, binary-not-found handling

### Phase 5 — File Watchers & Hot Reload

- [x] Register `uv.lock` file watcher (`workspace/didChangeWatchedFiles`)
- [x] Register `.python-version` file watcher
- [x] Register `pyproject.toml` change handler (staleness detection, workspace member changes)
- [x] Registry rebuild on `uv.lock`/`pyproject.toml` change: `rebuild_registry_and_resolve()` re-parses lock, re-resolves imports, republishes all diagnostics
- [ ] Tests: lock change triggers reparse, add/remove package updates diagnostics, Python version change

### Phase 6 — Hover Enrichment & Workspace Support

- [x] Hover on import: show package version, source, stub status from registry
- [x] Hover on workspace member import: show "Workspace member" + path (detected via non-site-packages path)
- [x] Hover on imports: show dependency classification (direct/dev/transitive) from uv registry
- [x] Parse `[tool.uv.workspace]` — extract member paths from glob patterns
- [x] Workspace member discovery: expand globs → find member `pyproject.toml` → extract package names
- [x] Wire workspace members into import resolver: `discover_workspace_members()` adds member src roots to search paths (after roots, before extraPaths)
- [ ] Multi-root LSP mapping for workspace members
- [x] Tests: hover content, workspace glob expansion, cross-member imports

### Phase 7 — Configuration & Editor Integration

- [x] Config key constants in `basilisk-common` (`UV`, `UV_ENABLED`, etc.)
- [x] Read from `basilisk.json` (`uv.stubSuggestions`, `uv.dependencyDiagnostics`) and `pyproject.toml` (`[tool.basilisk.uv]`)
- [x] `binary.rs` — uv binary resolution cascade: config path → `UV_PATH` env → `~/.cargo/bin/uv` → `~/.local/bin/uv` → OS PATH
- [x] VS Code: add `basilisk.uv.*` settings + commands to `package.json` and `extension.ts`
- [x] Neovim: uv config defaults, commands, and tests
- [x] Zed: uv settings in `default_workspace_config()` and tests
- [x] Update `LSP-ARCHITECTURE-SPEC.md` with uv integration architecture section, diagnostic codes table, binary resolution cascade, and hot reload docs
- [x] Tests: VS Code command registration, Neovim config defaults, Zed config wrapping
