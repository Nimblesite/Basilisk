# Basilisk Zed Extension — Plan

See [BASILISK-ZED-EXTENSION-SPEC.md](BASILISK-ZED-EXTENSION-SPEC.md) for the full technical specification.

## Implementation Plan

### Phase 1: Extension Scaffolding & LSP

Get the extension compiling, published to Zed's registry, and connecting to the Basilisk LSP.

- Create `basilisk-zed/` directory with `extension.toml`, `Cargo.toml`, `src/lib.rs`
- Implement `zed::Extension` trait with `language_server_command()`
- Binary resolution: check PATH, `~/.cargo/bin/basilisk`, cached download
- Implement GitHub release download for first-run installation
- Set installation status via `set_language_server_installation_status()`
- Implement `language_server_initialization_options()` — pass workspace root
- Implement `language_server_workspace_configuration()` — read Zed settings, map to Basilisk config
- Verify: LSP starts, diagnostics appear, completions work, hover works
- Test on macOS (aarch64) and Linux (x86_64)

### Phase 2: Tree-sitter Queries

Ship tree-sitter-python queries for full language support.

- Add `[grammars.python]` to `extension.toml` pointing to tree-sitter-python
- Create `languages/python/config.toml` with language metadata
- Create `highlights.scm` — keywords, builtins, decorators, f-strings, type annotations
- Create `brackets.scm` — `()`, `[]`, `{}`, string quotes
- Create `outline.scm` — functions, classes, methods for outline panel
- Create `indents.scm` — Python indentation rules
- Create `injections.scm` — SQL in strings, regex, docstrings
- Create `textobjects.scm` — Vim motions for functions, classes, arguments
- Create `runnables.scm` — `if __name__ == "__main__"`, pytest functions
- Decide: augment Zed's built-in Python support or replace it entirely
- Test: highlighting, outline panel, bracket matching, auto-indent

### Phase 3: Debugging (DAP)

Wire up Zed's debug adapter support to Basilisk's debug session manager.

- Implement `get_dap_binary()` — return basilisk binary path with debug-adapter args
- Create `debug_adapter_schemas/basilisk-debug.json` — launch/attach schema
- Implement `dap_request_kind()` — determine launch vs attach
- Implement `dap_config_to_scenario()` — map config to debug scenario
- Test: set breakpoint, F5, verify stepping works
- Test: attach mode
- Verify error handling: missing debugpy, missing Python

### Phase 4: Slash Commands (Profiling & Memory)

Add slash commands for profiling and memory analysis.

- Register `/profile`, `/profstop`, `/memleak`, `/memstop` in `extension.toml`
- Implement `run_slash_command()` dispatch
- `/profile [pid]` — send `basilisk/profiler/start`, return session info
- `/profstop` — send `basilisk/profiler/stop`, format hot functions/lines as markdown
- `/memleak` — send `basilisk/memory/start`, return memory session info
- `/memstop` — send `basilisk/memory/snapshot` + `basilisk/memory/diff`, format results
- Implement `complete_slash_command_argument()` for `/profile` — suggest running Python PIDs
- Include speedscope file path and browser URL in profiling output
- Include retention paths in memory leak output
- Test: `/profile` starts profiling, `/profstop` returns formatted results

### Phase 5: Polish & Publishing

- Add Basilisk theme to `themes/` directory (dark theme matching design system)
- Write extension description and README for Zed extension registry
- Set up CI: build WASM, test against Zed nightly
- Cross-platform testing: macOS aarch64, macOS x86_64, Linux x86_64, Linux aarch64
- Publish to Zed extension registry
- Add update mechanism: check for new basilisk releases on extension activation

---

## TODO List

### Phase 1: Extension Scaffolding & LSP

#### Project Setup
- [ ] Create `basilisk-zed/` directory
- [ ] Create `basilisk-zed/extension.toml` with id, name, version, schema_version, language_servers
- [ ] Create `basilisk-zed/Cargo.toml` with `crate-type = ["cdylib"]` and `zed_extension_api = "0.7.0"`
- [ ] Create `basilisk-zed/src/lib.rs` with `BasiliskExtension` struct
- [ ] Implement `zed::Extension` trait on `BasiliskExtension`
- [ ] Call `zed::register_extension!(BasiliskExtension)`

#### Binary Resolution
- [ ] Implement `resolve_binary()` helper method
- [ ] Check user-configured path from Zed settings
- [ ] Check `~/.cargo/bin/basilisk`
- [ ] Check `/usr/local/bin/basilisk`
- [ ] Check `/opt/homebrew/bin/basilisk` (macOS)
- [ ] Check extension data directory for cached download
- [ ] Implement GitHub release download fallback
- [ ] Detect platform via `zed::current_platform()` and match to release asset name
- [ ] Download binary via `zed::download_file()`
- [ ] Make binary executable via `zed::make_file_executable()`
- [ ] Set installation status via `zed::set_language_server_installation_status()`

#### LSP Integration
- [ ] Implement `language_server_command()` — return `zed::Command` with basilisk binary + "lsp" arg
- [ ] Implement `language_server_initialization_options()` — pass `workspaceRoot` from worktree
- [ ] Implement `language_server_workspace_configuration()` — read Zed settings, map to basilisk config
- [ ] Map `lsp.basilisk.settings.inlayHints.*` to basilisk config
- [ ] Map `lsp.basilisk.settings.ruff.*` to basilisk config
- [ ] Map `lsp.basilisk.initialization_options.python` to Python path config

#### Verification
- [ ] Verify: diagnostics appear on Python files with type errors
- [ ] Verify: completions triggered by dot and by typing
- [ ] Verify: hover shows type information
- [ ] Verify: go to definition works
- [ ] Verify: find references works
- [ ] Verify: rename symbol works
- [ ] Verify: inlay hints appear (parameter names, variable types)
- [ ] Verify: code actions appear (add type annotation, organize imports)
- [ ] Verify: semantic tokens work with `"semantic_tokens": "combined"` setting
- [ ] Verify: formatting works (via Ruff)
- [ ] Verify: document symbols appear in outline panel
- [ ] Test on macOS aarch64
- [ ] Test on Linux x86_64

### Phase 2: Tree-sitter Queries

- [ ] Add `[grammars.python]` to `extension.toml` with tree-sitter-python repo + rev
- [ ] Create `basilisk-zed/languages/python/config.toml`
- [ ] Set `name = "Python"`, `grammar = "python"`, `path_suffixes = ["py", "pyi"]`
- [ ] Set `line_comments = ["# "]`, `tab_size = 4`
- [ ] Create `highlights.scm` — full Python highlighting
- [ ] Capture: `@keyword`, `@keyword.control`, `@keyword.function`, `@keyword.return`
- [ ] Capture: `@function`, `@function.method`, `@function.builtin`
- [ ] Capture: `@type`, `@type.builtin`, `@variable`, `@variable.parameter`
- [ ] Capture: `@string`, `@string.escape`, `@string.special` (f-strings)
- [ ] Capture: `@number`, `@boolean`, `@constant`, `@comment`
- [ ] Capture: `@operator`, `@punctuation`, `@attribute` (decorators)
- [ ] Create `brackets.scm` — `@open` and `@close` for `()`, `[]`, `{}`
- [ ] Create `outline.scm` — `@item` for functions, classes, methods with `@name` and `@context`
- [ ] Create `indents.scm` — `@indent` for `:` blocks, `@end` for dedent
- [ ] Create `injections.scm` — SQL in string literals, regex patterns
- [ ] Create `textobjects.scm` — `@function.around/inside`, `@class.around/inside`, `@comment.around`
- [ ] Create `runnables.scm` — `@run` for `if __name__ == "__main__"` and `def test_*` functions
- [ ] Evaluate: does Zed's built-in Python conflict? Test augmenting vs replacing
- [ ] Verify: syntax highlighting matches expected colors
- [ ] Verify: outline panel shows correct symbol tree
- [ ] Verify: bracket matching works
- [ ] Verify: auto-indent works after `:` and inside blocks

### Phase 3: Debugging (DAP)

- [ ] Implement `get_dap_binary()` — resolve basilisk binary, return `DebugAdapterBinary`
- [ ] Set command to basilisk binary path
- [ ] Set args to `["debug-adapter"]`
- [ ] Set cwd from debug config
- [ ] Create `basilisk-zed/debug_adapter_schemas/basilisk-debug.json`
- [ ] Define `program` property (string, required)
- [ ] Define `args` property (array of strings)
- [ ] Define `cwd` property (string)
- [ ] Define `python` property (string, interpreter path)
- [ ] Define `justMyCode` property (boolean, default true)
- [ ] Define `stopOnEntry` property (boolean, default false)
- [ ] Define `console` property (enum: integratedTerminal, internalConsole)
- [ ] Add `[debug_adapters.basilisk-debug]` to `extension.toml` with schema path
- [ ] Implement `dap_request_kind()` — return Launch or Attach based on config
- [ ] Implement `dap_config_to_scenario()` — map config to `DebugScenario`
- [ ] Test: set breakpoint on a line, press F5, verify execution stops at breakpoint
- [ ] Test: step over, step into, step out
- [ ] Test: inspect variables in Variables panel
- [ ] Test: debug console evaluation
- [ ] Test: attach mode (connect to running debugpy)
- [ ] Test: error when debugpy not installed (verify actionable error message)
- [ ] Test: error when Python not found

### Phase 4: Slash Commands

#### Registration
- [ ] Add slash command declarations to `extension.toml`
- [ ] Register `/profile` slash command
- [ ] Register `/profstop` slash command
- [ ] Register `/profsnapshot` slash command
- [ ] Register `/memleak` slash command
- [ ] Register `/memstop` slash command
- [ ] Register `/memrefs` slash command (reference graph query)

#### Implementation
- [ ] Implement `run_slash_command()` dispatch on `command.name`
- [ ] `/profile` handler: parse optional PID arg, send `basilisk/profiler/start` via LSP
- [ ] Format `/profile` output: session ID, PID, Python version, start time
- [ ] `/profstop` handler: send `basilisk/profiler/stop` via LSP
- [ ] Format `/profstop` output: hot functions table, hot lines, speedscope path, browser URL
- [ ] `/profsnapshot` handler: send `basilisk/profiler/snapshot` via LSP
- [ ] Format snapshot output same as profstop but with "profiling continues" note
- [ ] `/memleak` handler: send `basilisk/memory/start` via LSP
- [ ] Format `/memleak` output: memory session ID, current memory, tracing started
- [ ] `/memstop` handler: send `basilisk/memory/snapshot` + `basilisk/memory/diff` via LSP
- [ ] Format `/memstop` output: top allocations, suspected leaks with confidence, growth summary
- [ ] `/memrefs` handler: parse type name arg, send `basilisk/memory/references` via LSP
- [ ] Format `/memrefs` output: retention path, node list with types and sizes, cycle warnings

#### Argument Completion
- [ ] Implement `complete_slash_command_argument()` dispatch
- [ ] `/profile` completion: list running Python process PIDs with command names
- [ ] `/memrefs` completion: list common Python types (DataFrame, dict, list, etc.)

#### Output Formatting
- [ ] Format all slash command output as markdown with `SlashCommandOutput`
- [ ] Use `SlashCommandOutputSection` to label sections (Hot Functions, Hot Lines, Retention Path)
- [ ] Include file:line references as clickable text
- [ ] Include speedscope.app URL for flamegraph viewing

#### Tests
- [ ] Test: `/profile` returns session info
- [ ] Test: `/profstop` returns formatted hot functions with percentages
- [ ] Test: `/memleak` starts memory tracking
- [ ] Test: `/memstop` returns leak report with confidence scores
- [ ] Test: `/memrefs DataFrame` returns retention path
- [ ] Test: argument completion returns PID suggestions

### Phase 5: Polish & Publishing

- [ ] Create `basilisk-zed/themes/basilisk-dark.json` matching design system colors
- [ ] Map Basilisk brand palette to Zed theme schema
- [ ] Write extension description for registry
- [ ] Set up CI workflow: build WASM target, run tests
- [ ] Test: macOS aarch64
- [ ] Test: macOS x86_64
- [ ] Test: Linux x86_64
- [ ] Test: Linux aarch64
- [ ] Implement version check: compare installed basilisk version against latest GitHub release
- [ ] Prompt user to update if newer version available
- [ ] Publish to Zed extension registry
- [ ] Verify: fresh install on clean machine downloads binary and activates correctly
