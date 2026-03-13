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

### Rust LSP Module (`crates/basilisk-lsp/src/debug.rs`)
- [x] Create `crates/basilisk-lsp/src/debug.rs` with `DebugSessionManager` struct
- [x] Implement `find_free_port()` (bind to `:0`, read assigned port)
- [x] Implement `wait_for_port()` (poll TCP connect, 50ms interval, 5s timeout)
- [x] Implement `start_session()` — spawn `python -m debugpy.adapter --port N`, wait for readiness
- [x] Implement `stop_session()` — kill child process, remove from session map
- [x] Implement `resolve_python()` — check `BASILISK_PYTHON`, workspace venv, system fallback
- [x] Implement `check_debugpy()` — async `python -c "import debugpy"` check
- [x] Define `DebugError` enum (SpawnFailed, PortAllocation, Timeout, DebugpyNotFound, PythonNotFound)

### LSP Server Integration (`crates/basilisk-lsp/src/server.rs`)
- [x] Add `debug_manager: DebugSessionManager` field to `LspServer`
- [x] Handle `basilisk.startDebugSession` in `execute_command`
- [x] Handle `basilisk.stopDebugSession` in `execute_command`
- [x] Register both commands in `initialize` capabilities
- [x] Add session cleanup on LSP `shutdown`

### VS Code Extension
- [x] Add `debuggers` contribution to `vscode-extension/package.json` (type `basilisk-debug`)
- [x] Add `configurationAttributes` for launch and attach modes
- [x] Add `configurationSnippets` and `initialConfigurations`
- [x] Add `BasiliskDebugAdapterFactory` class to `extension.ts`
- [x] Register factory in `activate()` via `vscode.debug.registerDebugAdapterDescriptorFactory`
- [x] Surface LSP errors (missing debugpy, missing Python) as VS Code notifications

### E2E Tests — LSP-level (`debug-integration.test.ts`)
- [x] Test: LSP advertises `startDebugSession` and `stopDebugSession` commands
- [x] Test: `basilisk-debug` type has correct configuration attributes
- [x] Test: `startDebugSession` spawns debugpy on a TCP port
- [x] Test: `stopDebugSession` kills the debugpy process
- [x] Test: stop nonexistent session returns `stopped: false`
- [x] Test: multiple concurrent debug sessions on different ports
- [x] Test: `startDebugSession` with bad Python path returns error
- [x] Test: full debug lifecycle — start, verify DAP handshake, stop

### E2E Tests — Real Debugger Stepping (`debug-integration.test.ts`)
- [x] Test: arithmetic — step through every line, assert x/y/z/w/result values
- [x] Test: string_ops — step through, assert string values, watch len/upper/startswith
- [x] Test: list_ops — step through, assert list contents, watch indexing/sum/len
- [x] Test: dict_ops — step through, assert dict contents, watch keys/values/membership
- [x] Test: nested_call — step INTO double(), verify call stack, step OUT back
- [x] Test: loop_and_accumulate — step through 5 iterations, verify accumulator each time
- [x] Test: conditional_branches — verify correct elif branch taken for x=42
- [x] Test: exception_handling — step through try/except, verify caught=True
- [x] Test: type_variety — verify int/float/bool/None/tuple/set/bytes type representations
- [x] Test: class_instance — verify p.x, p.y, magnitude(), type(p).__name__

### E2E Tests — Breakpoints, Watch, and Session Management
- [x] Test: continue between multiple breakpoints (arithmetic → string_ops)
- [x] Test: stack trace shows correct call hierarchy (double → nested_call)
- [x] Test: scopes show Locals with correct variable values
- [x] Test: watch expressions — arithmetic, boolean, isinstance, f-string, list comprehension
- [x] Test: hover evaluation context
- [x] Test: REPL evaluation context (Debug Console)
- [x] Test: debug session terminates cleanly after program ends
- [x] Test: attach to manually spawned debugpy server
- [x] Test: bad python path shows error
