# Whole-Module Analysis — Implementation Plan

> **Spec**: [WHOLE-MODULE-ANALYSIS-SPEC.md](WHOLE-MODULE-ANALYSIS-SPEC.md) — read before touching any code.

---

## Affected Components

| Component | Change |
|-----------|--------|
| `crates/basilisk-lsp/src/config.rs` | Add `AnalysisMode` enum + `analysis_mode` field to `BasiliskConfig` |
| `crates/basilisk-lsp/src/server.rs` | Replace `DashMap<Url, DocumentState>` with `WorkspaceIndex`; gate startup scan on mode |
| `crates/basilisk-lsp/src/workspace.rs` (new) | `WorkspaceIndex`, `FileEntry`, scan, invalidation, open/close logic |
| `vscode-extension/package.json` | Register `basilisk.analysisMode` setting |
| `vscode-extension/src/extension.ts` | Pass `analysisMode` via `InitializationOptions` |

## Reuse

- `collect_python_files()` (`server.rs:1024`) — move into `workspace.rs`
- `is_excluded()` (`server.rs:1064`) — move into `workspace.rs`
- `check_and_publish()` body — extract into `FileEntry::recheck()`
- `load_config()` in `config.rs` — extend, do not replace
- Rayon (`par_iter()`) for parallel startup scan — already in `Cargo.toml`

## Verification

1. `cargo build` — clean, no warnings
2. `cargo clippy` — passes
3. `cargo test -p basilisk-lsp` — all existing E2E and WS tests pass
4. Manual: open VS Code on a multi-file Python workspace; confirm diagnostics appear for **closed** files in the Problems panel (`wholeModule` mode)
5. Manual: set `basilisk.analysisMode` to `openFilesOnly`; confirm closed-file diagnostics disappear
6. New E2E test in `crates/basilisk-lsp/tests/lsp_ws_tests.rs`:
   - Start server with `wholeModule` mode
   - Send no `didOpen` notifications
   - Assert diagnostics are published for fixture files from the startup scan

---

## TODO

- [x] Add `AnalysisMode` enum (`OpenFilesOnly`, `WholeModule`, `CrossModule`) to `config.rs`
- [x] Add `analysis_mode: AnalysisMode` field to `BasiliskConfig` with default `WholeModule`
- [x] Deserialise `analysisMode` from `basilisk.json`, `[tool.basilisk]` in `pyproject.toml`, and `InitializationOptions`
- [x] Create `crates/basilisk-lsp/src/workspace.rs` with `WorkspaceIndex` and `FileEntry` structs
- [x] Implement `WorkspaceIndex::scan()` — returns `Vec<(Url, Vec<Diagnostic>)>`
- [x] Implement `WorkspaceIndex::set_open(path, text, version)` and `set_closed(path)`
- [x] Implement `WorkspaceIndex::reload_from_disk(uri)` — hash-gated invalidation
- [x] Move `collect_python_files()` and `is_excluded()` from `server.rs` into `workspace.rs`
- [x] Replace `DashMap<Url, DocumentState>` in `server.rs` with `WorkspaceIndex`
- [x] In `initialized()`: branch on `analysis_mode` — skip scan for `openFilesOnly`, run `workspace.scan()` for `wholeModule`/`crossModule`
- [x] Update `did_open` / `did_change` / `did_save` to call `workspace.set_open()`
- [x] Update `did_close` to call `workspace.set_closed()` (wholeModule) or clear diagnostics (openFilesOnly)
- [x] Update `did_change_watched_files` to skip open files; call `workspace.reload_from_disk()` for closed ones
- [x] Update all feature handlers to look up `FileEntry` from `WorkspaceIndex` instead of `DocumentState`
- [x] `cargo build` clean — fix any remaining compile errors
- [x] `cargo clippy` passes
- [x] `cargo test -p basilisk-lsp` all existing tests pass (133 total)
- [x] Write new WS E2E tests: startup scan, openFilesOnly no-scan, did_close behavior per mode
- [x] Write unit tests for `WorkspaceIndex` (24 tests covering set_open, get_text, set_closed, reload_from_disk, scan, all_resolved)
- [x] Fix `FileEntry.text` field — store raw source always, even when parse fails (enables completion on partial expressions)
- [ ] Add 150 ms debounce to file-watcher events
- [ ] Advertise `workspace.fileOperations` capabilities in `initialize` response when mode is not `openFilesOnly`
- [ ] Register `basilisk.analysisMode` setting in `vscode-extension/package.json`
- [ ] Pass `analysisMode` via `initializationOptions` in `vscode-extension/src/extension.ts`
- [ ] VSIX integration tests: prove wholeModule publishes diagnostics for closed files; prove openFilesOnly does not scan at startup
