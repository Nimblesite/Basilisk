# Basilisk LSP architecture {#LSPARCH}

This document records the shared, shipped LSP contract. Feature-specific behavior belongs
in the linked specs and editor-specific UI belongs in the VS Code, Zed, and Neovim specs.

## System architecture {#LSPARCH-SYSTEM}

`basilisk lsp` runs the parser, resolver, checker, formatter, and language features in one
process. VS Code, Zed, and Neovim communicate with it over LSP. External processes are
limited to features that require them, notably `debugpy`, `uv`, test runners, and profiling
helpers.

The analysis path is parser → resolver → checker. Workspace state, import graphs, and
resolved modules are retained by the server so feature handlers do not reparse every
request.

## Binary invocation {#LSPARCH-INVOKE}

```bash
basilisk lsp [--transport stdio|ws] [--port 8765]
```

`stdio` is the default transport. `BASILISK_LOG` controls server logging, which is written
to stderr so it cannot corrupt the protocol stream.

## Binary resolution {#LSPARCH-BINRES}

`shipwright.json` is the authority for VS Code runtime binaries and their resolution order.
The extension accepts an explicit user path and otherwise uses its bundled platform asset;
it does not silently substitute an unrelated executable from `PATH`. Zed and Neovim own
their installation flows in their editor specs.

## Shared configuration {#LSPARCH-CONFIG}

The server configuration model is `crates/basilisk-lsp/src/config.rs`. Editor manifests and
settings must map to that model rather than maintaining a second semantic configuration.
The stable shared surface includes the executable and Python paths, analysis mode, stub and
typeshed paths, formatter selection, inlay-hint switches, debugger settings, and the uv,
test, profiling, and memory namespaces. Detailed contracts live in their feature specs.

## Diagnostic scope {#LSPARCH-DIAGNOSTIC-SCOPE}

The LSP publishes the **union** of both command scopes — every `pep`-tagged
rule plus every configured analyze rule
([CHKARCH-COMMANDS](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-COMMANDS)) — through
one diagnostics stream. An IDE-level client option
(`initializationOptions.basilisk.analyze: false`, surfaced as an editor
setting) restricts publication to check scope. This is per-user editor
ergonomics: project configuration grades rules and never selects commands.

## Configuration seeding {#LSPARCH-CONFIG-SEEDING}

When the LSP opens a workspace root whose ancestor walk finds no
`[tool.basilisk]` table
([CHKARCH-CONFIG-DISCOVERY](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-DISCOVERY)),
it writes exactly one thing into the root's `pyproject.toml` (creating the
file when the project has none) — the two-line strict-by-default seed:

```toml
[tool.basilisk.rule-tags]
"basilisk" = "error"
```

PEP rules need no seeding — `check` always runs them
([CHKARCH-COMMANDS](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-COMMANDS)) — so this
one tag entry is the entire out-of-the-box configuration: every house rule on,
at `error`, in a file the user owns from that moment. Seeding happens once.
The LSP never re-seeds while any `[tool.basilisk]` table exists on the walk,
never edits the entry afterwards, and never resurrects anything the user
deletes — deleting the tag entry switches the house rules back off. The CLI
never seeds.

## Resolved environment reporting {#LSPARCH-RESOLVED-ENV}

The server — never the editor — is authoritative for what auto-detection actually
resolved (GitHub #153). Its `initialize` response reports
`capabilities.experimental.basilisk.resolvedEnvironment` with three slots —
`python` (the [LSPDEBUG-PYRES](LSP-DEBUG-INTEGRATION-SPEC.md#LSPDEBUG-PYRES) cascade
outcome, honouring a `basilisk.python` override, with bare command names located on
`PATH` so the reported path is concrete), `uv` (the
[uv binary cascade](#LSPARCH-UV-BINRES) outcome, honouring
`basilisk.uv.executablePath`), and `binary` (the running server itself:
`current_exe()` + crate version). Each slot is `{"path", "version"}` or `null` when
nothing usable exists; `version` is `null` when the `--version` probe fails but the
path is real. Versions come from running `<tool> --version` once at initialize.

Editors render a resolved tool as `<version> (<path>)`; with an empty (auto-detect)
setting the row shows the outcome — `auto-detect → 3.12.4 (/usr/bin/python3)`,
`auto-detect → none found`, or `auto-detect → awaiting server…` — never the bare
`auto-detect` placeholder. The Binary row renders only from this payload: populated
while a server is running, absent otherwise
([EXTACT-INFO-SERVER-INFO](EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO-SERVER-INFO)).

Implemented in `crates/basilisk-lsp/src/server/resolved_env.rs` (payload) and
`crates/basilisk-lsp/src/server/init.rs` (wiring); version probing shared with the
profiler's `version_via_command` (`crates/basilisk-lsp/src/profiler/processes.rs`).

## Configuration editor API {#LSPARCH-CONFIG-EDITOR}

The LSP is the configuration authority; clients render controls and relay typed
operations. The product contract, wire model, and selectors are specified in
[LSP-CONFIGURATION-EDITOR-SPEC.md](LSP-CONFIGURATION-EDITOR-SPEC.md).

### Protocol {#LSPARCH-CONFIG-EDITOR-PROTOCOL}

Method names are constants in `basilisk_common::configuration_editor` and their DTOs are
generated from `models/configuration_editor.td`. The server advertises the editor as
`capabilities.experimental.basilisk.configurationEditor` — pure presence, no version
negotiation: the editor ships with the server, so client and server can never disagree
about the protocol.

| Method | Kind | Purpose |
|---|---|---|
| `basilisk/configurationSnapshot` | request | Read one workspace root's effective configuration. |
| `basilisk/previewConfigurationChange` | request | Validate and analyse a hypothetical mutation without writing. |
| `basilisk/applyConfigurationChange` | request | Apply a revision-checked preview, reload, and recheck. |
| `basilisk/ruleOccurrences` | request | Page through locations selected by the all/codes/tags selectors. |
| `basilisk/configurationChanged` | notification | Tell clients to refresh after an effective change. |

Every request identifies its workspace root. Unknown roots, rules, tags, severities,
selectors, and stale revisions are errors; they never fall back to a different root
or to defaults.

A mutation sets or removes one rule entry or one tag entry — nothing else
exists: no preset, inherit, or native intents, and setting `disabled` on a
`pep`-tagged rule is a request error
([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)).
Mass fixing stays the standalone `basilisk.fixWorkspace` execute command; a
supplied `{ rootUri }` is validated and restricts the edit to that active root.

### Transaction and refresh {#LSPARCH-CONFIG-EDITOR-TRANSACTION}

Snapshot and preview preload eligible closed files for the selected root, even
when diagnostics are otherwise published only for open files. Preview resolves
the one active source, validates and expands the selector, builds an in-memory
patch, and returns normalized changes plus full-root diagnostic impact. Apply
accepts only that preview and its unchanged root/base revision, asks the client
to perform one `WorkspaceEdit`, then reloads, rechecks, republishes, and sends
`basilisk/configurationChanged`. External config-file changes use the same
refresh tail.

Rule and tag entries are stored in `pyproject.toml` (`[tool.basilisk]`) config
files; the nearest folder table that decides a rule wins
([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)).

### Errors {#LSPARCH-CONFIG-EDITOR-ERRORS}

Protocol failures are JSON-RPC errors with a stable `data.kind`. Current
handlers distinguish invalid input/configuration, unknown selectors, revision
conflicts, read-only sources, rejected edits, and expired previews.
Malformed-source errors include the source URI so a client can open it for
repair.

## Command registration rule {#LSPARCH-CMDREG}

The server advertises every server-owned command from
`basilisk_common::commands::ALL` through `executeCommandProvider`. Editor extensions must
not register those names themselves. Client-only commands such as restart or show-output
remain editor-owned, and tests must wait for the LSP handshake before checking advertised
commands.

### Modules toolbar contract (VS Code) {#VSIX-MODULE-EXPLORER-TOOLBAR}

The MODULES title bar contributes refresh, view toggle, filter, and flat-view sort in that
order. VS Code supplies Collapse All through `showCollapseAll`; no duplicate command is
contributed. Mutating and server-control actions live in separate overflow groups, and Fix
All remains behind `basilisk.experimental.fixAll`. The extension contract tests enforce the
ordering, visibility conditions, and unique icons.

## Custom commands {#LSPARCH-CMDS}

`crates/basilisk-common/src/lib.rs` is the command-name registry and
`crates/basilisk-lsp/src/server/` contains dispatch. Feature specs own request/response
semantics: formatting, debugging, mass fix/adoption, refactoring, stubs, tests, uv,
profiling/memory, and activity-panel queries. Keeping the list in code prevents this document
from becoming a stale second registry.

### Custom notifications {#LSPARCH-NOTIFS}

Notification names are canonical in `basilisk_common::notifications`. They cover module
changes, completed workspace scans, profiler progress, and memory timelines. In particular,
`basilisk/scanComplete` lets clients settle an empty workspace without guessing from the
absence of diagnostics.

### Activity-panel data model {#LSPARCH-DATAMODEL}

The wire model is implemented in `crates/basilisk-lsp/src/server/activity_panel/` and
consumed by the editor clients. Field meanings and rendering behavior are specified once in
[EXTENSION-ACTIVITY-PANEL-SPEC.md](EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-DATA-MODEL).

## DAP TCP proxy {#LSPARCH-DAPPROXY}

Editors connect to the `debugpy` endpoint returned by Basilisk and apply their DAP proxy
compatibility behavior. Session lifecycle is specified in
[LSP-DEBUG-INTEGRATION-SPEC.md](LSP-DEBUG-INTEGRATION-SPEC.md); editor implementation details
belong in the VS Code, Zed, and Neovim specs.

## Analysis architecture {#LSPARCH-ARCH}

### Three-phase pipeline {#LSPARCH-ARCH-PIPELINE}

`basilisk_parser::parse_source` produces the Ruff AST, `basilisk_resolver::resolve` produces
a `ResolvedModule`, and `basilisk_checker::check` produces diagnostics. LSP features reuse
the same resolved state.

### Resolved module {#LSPARCH-ARCH-RESOLVED}

`ResolvedModule` is defined by `basilisk-resolver`. It carries functions, classes, variables,
imports, calls, type parameters, attribute accesses, source text, and byte spans. Feature
handlers consume this structure instead of scanning source when semantic data is available.

### Server module structure {#LSPARCH-ARCH-MODSTRUCT}

The crate is split by protocol feature (`hover`, `definition`, `references`, `completion`,
`signature`, `inlay_hints`, `semantic_tokens`, hierarchy, folding, selection, and code lens),
with command handlers under `server/`. `server/init.rs` advertises capabilities and
`server/mod.rs` coordinates document/workspace state.

### Cached analysis {#LSPARCH-ARCH-CACHE}

Document and workspace state retain `Arc<ResolvedModule>` values and diagnostics. Changes
replace the affected state; read-only feature requests reuse it.

### Runtime stack sizing {#LSPARCH-ARCH-STACK}

All production entry points run analysis on the stack-sized helpers in
`crates/basilisk-lsp/src/runtime.rs`. The CLI uses `run_with_analysis_stack`; stdio and
WebSocket LSP paths use `block_on_with_analysis_stack`. The parser's depth guard rejects
pathological operator chains before recursive analysis. Binary-level regression tests cover
deep expressions on both CLI and LSP paths.

## LSP features {#LSPARCH-FEATURES}

Advertised capabilities are canonical in `crates/basilisk-lsp/src/server/init.rs`; the
sections below identify their implementation contracts without duplicating test inventories.

### Symbol lookup {#LSPARCH-FEATURES-FINDSYM}

`util::find_symbol_at_offset` maps byte offsets to resolver symbols and is shared by
navigation and display features.

### Hover {#LSPARCH-FEATURES-HOVER}

Hover presents the resolved symbol signature and relevant Basilisk diagnostics; unknown
inferred pieces are represented explicitly rather than fabricated.

### Go to definition {#LSPARCH-FEATURES-DEFINITION}

Definition resolves symbols through spans and, in cross-module mode, the workspace import
graph. Unsupported cases return no location.

### Document symbols {#LSPARCH-FEATURES-DOCSYM}

Document symbols expose the resolved module's classes, functions, methods, fields, imports,
and module variables as an outline.

### Signature help {#LSPARCH-FEATURES-SIGHELP}

Signature help triggers on `(` and `,`, tracks the active argument, and omits implicit
`self`/`cls` receivers.

#### VS Code signature-help contract {#LSPARCH-FEATURES-SIGNATURE-HELP}

The VS Code integration consumes the same server capability; it has no parallel signature
engine.

### References {#LSPARCH-FEATURES-REFS}

References use resolver/scoping data and honor `includeDeclaration`.

### Rename {#LSPARCH-FEATURES-RENAME}

Prepare-rename validates the symbol and range. Rename is scope-aware in one file and adds
import-graph edits when cross-module analysis is enabled. The full refactoring contract is
in [LSP-REFACTORING-SPEC.md](LSP-REFACTORING-SPEC.md#REFACTOR-RENAME).

### Completion {#LSPARCH-FEATURES-COMPLETION}

Completion combines visible symbols, members, imports, builtins, prefixes, and keyword
arguments. Its implementation is split under `completion/`.

### Code actions {#LSPARCH-FEATURES-CODEACTIONS}

Code actions expose diagnostic fixes, suppression, import hygiene, stub helpers, mass-fix
actions, and deterministic refactors when their preconditions hold. Each action must be
derived from current diagnostics/source and return explicit edits or commands.

### Execute command {#LSPARCH-FEATURES-EXECCMD}

`workspace/executeCommand` dispatches only names advertised through
`basilisk_common::commands::ALL`.

### Inlay hints {#LSPARCH-FEATURES-INLAYHINTS}

Hints cover inferred variable/return types, generic parameters, and call-site parameter
names. Configuration gates are applied before results are returned.

### Semantic tokens {#LSPARCH-FEATURES-SEMTOKENS}

The token legend and modifiers are exported by `semantic_tokens.rs` and advertised unchanged
during initialization.

### Document highlight {#LSPARCH-FEATURES-HIGHLIGHT}

Highlights classify the selected symbol's declaration/definition as write and uses as read.

### Workspace symbols {#LSPARCH-FEATURES-WSSYM}

Workspace symbol search filters the indexed symbols from analysed documents.

### Formatting {#LSPARCH-FEATURES-FORMAT}

Document and range formatting run the embedded Ruff formatter in-process. Configuration,
provenance, and import hygiene are specified in
[LSP-FORMATTING-SPEC.md](LSP-FORMATTING-SPEC.md).

### Folding ranges {#LSPARCH-FEATURES-FOLDING}

Folding uses resolved function/class spans and import blocks.

### Selection ranges {#LSPARCH-FEATURES-SELECTION}

Selection ranges expand from the identifier through containing semantic spans to the module.

### Call hierarchy {#LSPARCH-FEATURES-CALLHIER}

Call hierarchy is derived from resolved functions and call sites, with workspace results
available when the analysis mode supplies them.

### Type hierarchy {#LSPARCH-FEATURES-TYPEHIER}

Type hierarchy derives parents and children from resolved class bases.

### Code lens {#LSPARCH-FEATURES-CODELENS}

Code lenses report reference counts for functions and classes.

## uv integration {#LSPARCH-UV}

The canonical behavior is [LSP-UV-INTEGRATION-SPEC.md](LSP-UV-INTEGRATION-SPEC.md).

### Detection and registry {#LSPARCH-UV-DETECT}

Filesystem signals and `uv.lock`/`pyproject.toml` build the package registry without spawning
uv. That registry feeds dependency classification, workspace roots, diagnostics, hover, and
code actions.

### Hot reload {#LSPARCH-UV-HOTRELOAD}

Successful uv commands and relevant file changes rebuild project state, re-resolve imports,
and republish affected diagnostics without restarting the server.

### uv binary resolution {#LSPARCH-UV-BINRES}

The executable is needed only for uv commands. The current resolver and its supported
overrides are defined in `crates/basilisk-lsp/src/uv_commands.rs` and the uv spec.

### uv diagnostics {#LSPARCH-UV-DIAGCODES}

Rule codes, defaults, and gates are owned by the checker rule catalog and summarized in the
uv spec; this overview does not maintain a duplicate table.

## Stub resolution and type provenance {#LSPARCH-STUBS}

See [CHECKER-STUB-RESOLUTION-SPEC.md](CHECKER-STUB-RESOLUTION-SPEC.md).

## Analysis modes {#LSPARCH-MODES}

See [LSP-ANALYSIS-MODES-SPEC.md](LSP-ANALYSIS-MODES-SPEC.md).

## Editor-specific specs {#LSPARCH-EDITORS}

- [VSIX-SPEC.md](VSIX-SPEC.md)
- [ZED-SPEC.md](ZED-SPEC.md)
- [NEOVIM-SPEC.md](NEOVIM-SPEC.md)

## Testing {#LSPARCH-TESTING}

Protocol behavior is tested in `crates/basilisk-lsp/tests/`; focused unit tests live beside
feature modules, and editor suites test client wiring. Tests should exercise the public
protocol or pure feature functions and must not create a second command/capability registry.
