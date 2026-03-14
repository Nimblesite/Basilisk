# Basilisk Zed Extension — Plan

See [ZED-SPEC.md](../specs/ZED-SPEC.md) for the full technical specification.

---

## TODO

### Phase 1: Extension Scaffolding & LSP

- [x] Create `basilisk-zed/` with `extension.toml`, `Cargo.toml`, `src/lib.rs`
- [x] Implement `zed::Extension` trait with `language_server_command()`
- [x] Implement `language_server_initialization_options()` and `language_server_workspace_configuration()`
- [x] Binary resolution: user config, `BASILISK_PATH` env var, `~/.cargo/bin/basilisk`
- [ ] Binary resolution: GitHub release download fallback
- [x] Verify: diagnostics, completions, hover, go-to-def, references, rename, inlay hints, code actions, semantic tokens, formatting, document symbols (E2E tests in `zed_extension_e2e_tests.rs`)
- [ ] Test on macOS aarch64 and Linux x86_64

### Phase 2: Tree-sitter Queries

- [ ] Add `[grammars.python]` to `extension.toml`
- [ ] Create `languages/python/` with `config.toml`, `highlights.scm`, `brackets.scm`, `outline.scm`, `indents.scm`, `injections.scm`, `textobjects.scm`, `runnables.scm`
- [ ] Evaluate: augment vs replace Zed's built-in Python support
- [ ] Verify: highlighting, outline panel, bracket matching, auto-indent

### Phase 3: Debugging (DAP)

- [x] Create `debug_adapter_schemas/basilisk-debug.json` (launch/attach schema)
- [ ] Implement `get_dap_binary()` — resolve basilisk binary, return `DebugAdapterBinary`
- [ ] Implement `dap_request_kind()` and `dap_config_to_scenario()`
- [ ] Test: breakpoints, stepping, variables, debug console, attach mode, error handling

### Phase 4: Slash Commands (Profiling & Memory)

- [x] Register `/profile`, `/profstop`, `/profsnapshot`, `/memleak`, `/memstop`, `/memrefs` slash commands
- [x] Implement `run_slash_command()` dispatch (stubs returning placeholder messages)
- [x] Implement `complete_slash_command_argument()` for PID and type suggestions
- [ ] Wire slash commands to actual LSP profiler/memory commands (blocked on profiling engine)
- [ ] Format output as real markdown with hot functions, retention paths, etc.

### Phase 5: Polish & Publishing

- [ ] Create Basilisk theme (`themes/basilisk-dark.json`)
- [ ] Set up CI: build WASM, test against Zed nightly
- [ ] Cross-platform testing: macOS aarch64/x86_64, Linux x86_64/aarch64
- [ ] Publish to Zed extension registry
- [ ] Version check and update mechanism
