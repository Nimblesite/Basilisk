# Basilisk LSP architecture {#LSPARCH}

This document records the shared, shipped LSP contract. Feature-specific behavior belongs
in the linked specs and editor-specific UI belongs in the VS Code, Zed, and Neovim specs.

## System architecture {#LSPARCH-SYSTEM}

`basilisk lsp` runs the parser, resolver, checker, formatter, and language features in one
process. VS Code, Zed, and Neovim communicate with it over LSP. External processes are
limited to features that require them, notably `debugpy`, `uv`, test runners, and profiling
helpers. On startup the server acquires the step-3 typeshed source required by
the pinned typing order
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst),
[STUBRES-TYPESHED](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED)). Automatic
acquisition resolves an exact SHA and downloads an HTTPS archive; Basilisk never
clones the repository, and an explicit custom path requires no network.

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
The stable shared surface includes the executable and Python paths, analysis mode, stub
paths, the three typeshed source settings
(`typeshed-path`, `typeshed-commit`, `typeshed-store-path` —
[STUBRES-TYPESHED-CONFIG](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-CONFIG)),
formatter selection, inlay-hint switches, debugger settings, and the uv, test, profiling,
and memory namespaces. The custom path implements the pinned typing specification's
step-3 "canonical source" option
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
Detailed contracts live in their feature specs.

There is ONE project configuration
([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)), shared by
every surface and identical across IDEs: the CLI reads it fresh on every run; the LSP
watches it live; any future server surface (e.g. MCP) MUST consume the same model. The
LSP watches each root's active source itself, along with the `uv.lock` and
`.python-version` environment sources (`configuration_editor/watch.rs`,
`CONFIG_WATCH_POLL_MS`) — it MUST NOT depend on client file watchers (Zed has none).
Every change — editor-UI apply, open-buffer edit, or external disk edit/create/delete —
runs one shared refresh tail (`configuration_editor/transaction.rs`): reload the
effective document, update root config **and invalidate every derived config cache**,
recheck, republish, push `basilisk/configurationChanged`. The server pushes; clients
never poll; no configuration change ever requires a restart; the UI is never left stale.
Client `didChangeWatchedFiles` events are only a latency optimization; both paths share
one per-source disk baseline so a change refreshes exactly once
(E2E: `tests/lsp/ws_test_configuration_watch.rs`).

The refreshed in-memory root config is **authoritative over disk**: an applied
editor-UI change or an open, unsaved config buffer decides even while the root's
on-disk file still holds older content. Per-file config discovery therefore never
re-reads a root's own config file — inside a root, the discovered ancestor chain is
bounded to directories strictly below it (`workspace.rs` `config_for_file`,
`load_basilisk_config_below`); only nested child configs come from disk. Re-merging
the root's disk file over the in-memory config would silently resurrect stale state
between the apply and the client's write reaching disk.

Two tiers, stated once: **project configuration** (the watched files above) defines
semantics and is fully live. **Editor session settings** (`initializationOptions` /
`workspace/didChangeConfiguration` — formatter selection, analysis-mode override, inlay
hints, the analyze/enabled toggles) are per-user ergonomics delivered by the client;
they never define project semantics and their capability advertisements are fixed at
`initialize` unless a feature spec says otherwise.

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
it writes exactly one seed into the root's `pyproject.toml` (creating the
file when the project has none) — the strict-by-default rule tag plus the
binary's bundled typeshed pin:

```toml
[tool.basilisk]
typeshed-commit = "<bundled 40-hex sha>"

[tool.basilisk.rule-tags]
"basilisk" = "error"
```

PEP rules need no seeding — `check` always runs them
([CHKARCH-COMMANDS](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-COMMANDS)) — so this
is the entire out-of-the-box configuration: every house rule on, at `error`,
and the workspace pinned to the typeshed commit the binary already bundles
(`bundled_commit_sha()`), in a file the user owns from that moment. The pin
makes a freshly-opened workspace reproducible — never `typeshed_source_unpinned`
([STUBRES-TYPESHED-WARN](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN))
— without changing which stubs are resolved and without network access: the
bundled commit is complete inside the binary, so the pin cannot produce a
`NO SOURCE` state. The seed only runs when no `[tool.basilisk]` table exists,
so no user-set `typeshed-path` can be present for the pin to conflict with.
Seeding happens once. The LSP never re-seeds while any `[tool.basilisk]`
table exists on the walk, never edits the entries afterwards, and never
resurrects anything the user deletes — deleting the tag entry switches the
house rules back off, and deleting the pin returns the workspace to floating
resolution. The CLI never seeds.

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
setting the row shows the outcome — `auto-detect → X.Y.Z (/usr/bin/python3)`,
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

A mutation sets or removes one rule entry, one tag entry, or one allowlisted
Typeshed setting
([STUBRES-TYPESHED-CONFIG](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-CONFIG))
— nothing else exists: no preset, inherit, or native intents, and setting
`disabled` on a `pep`-tagged rule is a request error
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
refresh tail ([LSPARCH-CONFIG](#LSPARCH-CONFIG)).

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

Every path that tears a client down — extension `deactivate()`, `store.reset()`, and the
client-owned `basilisk.restartServer` — goes through one shutdown helper
(`vscode-extension/src/lsp-client-stop.ts`), never `client.stop()` directly, because
`stop()` rejects for any client state but `Running`:

- A client that is **`Starting`** has already spawned its server process, yet `isRunning()`
  reports `false` for it. Guarding on `isRunning()` therefore either lets the rejection
  escape the shutdown, or drops a client whose server keeps running — a zombie that goes on
  publishing into its own diagnostics collection (GitHub #264). The helper awaits the
  in-flight `start()` — which returns the existing start rather than beginning a second one
  — and only then shuts down.
- **Concurrent shutdowns of one client collapse into one.** `deactivate()` stops the client
  and then calls `reset()`, which also wants it gone; the second must join the first rather
  than shut down a client already `Stopping` and take its rejection.
- **Shutdown never rejects.** A client that failed to start is already stopped, and a
  shutdown error must not take down the deactivation around it.

The `Starting` window is narrow on Linux and wide on win32, where spawning the server binary
is slow — see [VSIX-CI-PLATFORM-COVERAGE-CLASSES]. Tests accordingly budget a client restart
with `LSP_RESTART_WAIT_MS`, not the module-generic one-second `WAIT_MS`.

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

### AST mandate {#LSPARCH-ARCH-AST}

**Normative for every feature handler and code action in this crate.** Source
text enters this pipeline once, at `basilisk_parser::parse_source`. Everything
downstream consumes the AST, the `ResolvedModule`, and the binding table
([RESOLV-CANONICAL](CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL)).

A handler may **never** recover Python structure by matching characters:
locating a definition by a line's prefix, finding the import block by scanning
for `import ` / `from `, hand-lexing identifiers, maintaining a Python keyword
table in Rust, or inferring scope from indentation bytes. Such code matched
inside docstrings and string literals, missed multi-line constructs, and changed
behaviour when the file was reformatted; all of it has been deleted rather than
patched. The clause above says *when semantic data is available* — that is not a
licence to fall back to scanning when it is not. A handler without the semantic
data it needs returns nothing.

`ResolvedModule::source` is for rendering: computing an edit range or a
diagnostic span once the target node is known, and quoting code back to the
user. It is not an input to a decision.

Permitted, because none of it infers Python structure: line and column geometry;
Basilisk's own `# basilisk:` directives, which are genuinely comments the AST
does not carry; text Basilisk itself rendered and parses back for display, such
as a formatted stub signature; and searching for the **user's own** selected
identifier during rename, subject to
[REFACTOR-RENAME-SCOPE](LSP-REFACTORING-SPEC.md#REFACTOR-RENAME-SCOPE).

See [REFACTOR-AST](LSP-REFACTORING-SPEC.md#REFACTOR-AST) for the refactoring
surface, and [REFACTOR-STATUS](LSP-REFACTORING-SPEC.md#REFACTOR-STATUS) for
which actions are currently unshipped as a result.

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

**A hover answers for the symbol the cursor is actually on.** How the identifier is reached
decides how it is resolved, and the two forms must never be mixed:

- A **free name** (`getLogger`, `MyClass`) resolves against the module's own bindings —
  local definitions first, then the names its imports bind.
- A **member access** (`logger.error`, `os.getcwd`, `" ".join`) resolves *through its
  receiver*: the module the receiver binds, a class in the local hierarchy, an external
  (stub or `py.typed`) class, then a built-in type. The receiver is typed from its own
  declaration only — its annotation, the literal assigned to it, or the return type of the
  call that produced it.

`ResolvedModule::imported_symbols` is keyed by bare name and a plain `import os` publishes
every member of `os` into it, so consulting it for a member access would answer
`logger.error` with whichever imported module last exported the word `error`. It may
therefore only be consulted for a name the module genuinely **binds** — verified by the
resolved file the binding import points at, not by the name alone. The same rule governs
the provenance annotation: a symbol is attributed to an import only when an import bound
it. **A receiver nothing can type yields no hover** — silence is correct where a confident
wrong answer is not.

Each rendered symbol states, in order, what kind of thing it is (`(function)`, `(method)`,
`(class)`, `(variable)` — the same vocabulary local and imported symbols share), its exact
declared shape (every overload of an overload set, and a class's declared bases), its own
documentation when the defining module carries any (`.pyi` stubs, Typeshed included, carry
none), and where the declaration was read from: its module, its provenance, and its source
path. Absent pieces are omitted, never invented.

A non-PEP diagnostic's
hover section additionally carries a **Configure Severity** command link
(`command:basilisk.openConfigurationEditor` with a `{ "rule": <code> }` argument) that
deep-links into the configuration editor focused on the rule
([CONFIGEDITOR-VSIX-EXPERIENCE](LSP-CONFIGURATION-EDITOR-SPEC.md#CONFIGEDITOR-VSIX-EXPERIENCE));
PEP rules are graded by the typing spec and never disabled, so they get no link.

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
