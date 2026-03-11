# Basilisk Debug Integration — Plan

See [BASILISK-DEBUG-INTEGRATION-SPEC.md](BASILISK-DEBUG-INTEGRATION-SPEC.md) for the full technical specification.

## Implementation Plan

### Day 1: LSP debug module (Rust)
- Add `debug.rs` to `basilisk-lsp` with `DebugSessionManager`
- Add `resolve_python()` and `check_debugpy()`
- Wire `basilisk.startDebugSession` and `basilisk.stopDebugSession` into `execute_command`
- Register the new commands in `initialize` capabilities
- Test: send raw LSP request, verify debugpy spawns on the returned port

### Day 2: VS Code integration
- Add `debuggers` contribution to `vscode-extension/package.json`
- Add `BasiliskDebugAdapterFactory` to `extension.ts` (~20 lines)
- Test: open a `.py` file, F5, verify breakpoints work

### Day 3: Polish
- Error handling: missing debugpy, missing Python, port conflicts
- Session cleanup: kill debugpy when debug session ends or LSP shuts down
- Test attach mode
- Verify Zed can use the same LSP command

---

## TODO List

- [ ] Create `crates/basilisk-lsp/src/debug.rs` with `DebugSessionManager` struct
- [ ] Implement `find_free_port()` (bind to `:0`, read assigned port)
- [ ] Implement `wait_for_port()` (poll TCP connect, 50ms interval, 5s timeout)
- [ ] Implement `start_session()` — spawn `python -m debugpy.adapter --port N`, wait for readiness
- [ ] Implement `stop_session()` — kill child process, remove from session map
- [ ] Implement `resolve_python()` — check `BASILISK_PYTHON`, workspace venv, system fallback
- [ ] Implement `check_debugpy()` — async `python -c "import debugpy"` check
- [ ] Define `DebugError` enum (SpawnFailed, PortAllocation, Timeout, DebugpyNotFound)
- [ ] Add `debug_manager: DebugSessionManager` field to `LspServer`
- [ ] Handle `basilisk.startDebugSession` in `execute_command`
- [ ] Handle `basilisk.stopDebugSession` in `execute_command`
- [ ] Register both commands in `initialize` capabilities
- [ ] Add session cleanup on LSP `shutdown`
- [ ] Add `debuggers` contribution to `vscode-extension/package.json` (type `basilisk-debug`)
- [ ] Add `configurationAttributes` for launch and attach modes
- [ ] Add `configurationSnippets` and `initialConfigurations`
- [ ] Add `BasiliskDebugAdapterFactory` class to `extension.ts`
- [ ] Register factory in `activate()` via `vscode.debug.registerDebugAdapterDescriptorFactory`
- [ ] Surface LSP errors (missing debugpy, missing Python) as VS Code notifications
- [ ] Test: launch debug session, hit breakpoint, inspect variables
- [ ] Test: attach to remote debugpy process
- [ ] Test: missing debugpy produces actionable error
- [ ] Test: missing Python interpreter produces actionable error
- [ ] Test: LSP shutdown kills orphaned debugpy processes
