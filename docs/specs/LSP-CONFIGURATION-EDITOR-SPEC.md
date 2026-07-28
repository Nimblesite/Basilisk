# Configuration editor {#CONFIGEDITOR}

This contract composes:

- the configuration model and command partition from [CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL) and [CHKARCH-COMMANDS](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-COMMANDS), with discovery from [CHKARCH-CONFIG-DISCOVERY](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-DISCOVERY);
- LSP seeding from [LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING) and diagnostic scope from [LSPARCH-DIAGNOSTIC-SCOPE](LSP-ARCHITECTURE-SPEC.md#LSPARCH-DIAGNOSTIC-SCOPE);
- canonical rule tags from [CHKTAG](CHECKER-RULE-TAGGING-SPEC.md#CHKTAG);
- shared methods from [LSPARCH-CONFIG-EDITOR](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-EDITOR); and
- VS Code hosting from [VSIX-CONFIGURATION-EDITOR](VSIX-SPEC.md#VSIX-CONFIGURATION-EDITOR).

The LSP owns the rule catalog, configuration parsing, impact analysis, and mutation. Clients render server data, request server operations, and apply the returned edit. They do not maintain another rule catalog or write project configuration directly.

The editor is a veneer over the configuration model, never an extension of it. Its rule table requests exactly four mutations: set/remove a rule or tag entry. The separate Typeshed and Caching panels use allowlisted set/remove-setting mutations for the keys in [STUBRES-TYPESHED-CONFIG](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-CONFIG) and [CHKCACHE-CONFIG](CHECKER-CACHE-SPEC.md#CHKCACHE-CONFIG). The five navigation views (Overview, Rules, Adoption, Path Overrides, Project) never widen that vocabulary: each renders server-computed state and drives only those mutations, allowlisted Typeshed actions, or standalone adopt/safe-fix commands, so every editor concept beyond a rule entry, a tag entry, and an allowlisted Typeshed or caching setting — snapshots, previews, occurrence pages, the debt summary, the path inventory, the read-only in-session cache report — exists only on the wire, never as a new kind of config line.

## Rule catalog and tags {#CONFIGEDITOR-TAGS}

The checker registry supplies each rule's code, title, summary, documentation URL, and canonical tags ([CHKTAG](CHECKER-RULE-TAGGING-SPEC.md#CHKTAG)). The LSP exposes that catalog with each rule's per-rule entry (if any), its resolved effective severity, and its current diagnostic count.

Tags serve two roles, both explicit. As *facets*, they group the catalog for navigation — provenance (`pep`, `basilisk`), reserved PEP category, and descriptive policy tags are flat facets and one rule may carry several. As *entries*, a tag written into `[tool.basilisk.rule-tags]` grades every rule carrying it — one visible line of config, never an implicit switch. The snapshot reports each tag's entry alongside its facet data.

## LSP operations {#CONFIGEDITOR-OPERATIONS}

All configuration-editor requests require an explicit active-workspace `rootUri`:

- `basilisk/configurationSnapshot` returns the root config document URI and revision, the active source (URI/exists/read-only), rule/tag states, the discovered per-folder path overrides, a server-computed debt summary, the real configuration problems, the server-described Typeshed settings and active status, and the server-described caching state for both layers ([§LSPCFGED-CACHE](#LSPCFGED-CACHE)).
- `basilisk/previewConfigurationChange` validates the base revision, builds a validated in-memory patch from the requested mutations, and reruns checking against that hypothetical config, returning the resolved per-rule effective-severity changes, the exact persisted Typeshed and caching setting changes, and before/after impact.
- `basilisk/applyConfigurationChange` consumes a cached preview identified by root and preview ID; the server rejects it if the preview's pinned base revision no longer matches the current document. It asks the client to apply one configuration edit, reloads and rechecks the root, republishes diagnostics, emits `basilisk/configurationChanged`, and returns a fresh snapshot.
- `basilisk/ruleOccurrences` returns URI/range/code-ordered pages selected by the all/codes/tags selectors. The opaque cursor resumes the stable result and the server accepts limits from 1 to 1000.
- `basilisk/typeshedAction` accepts only `DownloadLatest`, `DownloadPinned`, or `ViewLicense`; it returns an ordinary config preview (the new pin), a refreshed snapshot, or a safe read-only license document respectively.

A mutation is `SetRule`, `RemoveRule`, `SetTag`, `RemoveTag`, `SetTypeshedSetting`, `RemoveTypeshedSetting`, `SetCacheSetting`, or `RemoveCacheSetting`; the setting variants accept only the typed keys in [§LSPCFGED-TYPESHED](#LSPCFGED-TYPESHED) and [§LSPCFGED-CACHE](#LSPCFGED-CACHE). Every setting value crosses the wire as text, so the mutation union keeps one shape; the server parses `"true"`/`"false"` for `CacheEnabled` and rejects anything else, so no value the TOML parser would silently drop reaches the document. Requesting `disabled` for a `pep`-tagged rule is an error: PEP rules are graded, never disabled ([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)).

Snapshot, preview, and occurrence inventory cover the complete selected root, even when analysis is configured to publish only open files. Open buffers stay authoritative; eligible closed files are loaded from disk into the server index without publishing additional diagnostics.

The `basilisk.disableRule` command writes an explicit `disabled` rule entry through the same validated, root-aware mutation service, and is rejected for `pep`-tagged rules. Configuration watching is server-owned ([LSPARCH-CONFIG](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG)).

Unknown roots, rule codes, tags, severities, selectors, or Typeshed/caching keys/values/combinations, plus pep-disable requests, stale revisions, malformed configuration, expired previews, and client-rejected edits are request errors ([LSPARCH-CONFIG-EDITOR-ERRORS](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-EDITOR-ERRORS)).

## Wire model {#CONFIGEDITOR-MODEL}

The editor protocol's design source is [`models/configuration_editor.td`](../../models/configuration_editor.td). It builds on the core configuration model in [`models/configuration.td`](../../models/configuration.td) ([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)) — `RuleCode`, `RuleTag`, `RuleSeverity` — and **MUST NOT add anything to it**: the two models are separate files precisely so editor machinery can never contaminate the config model. Language DTOs are regenerated from the two files together.

The protocol is deliberately small:

- a snapshot is the root, config URI/revision, the active source (URI/exists/read-only), rule/tag states, the discovered per-folder path overrides, a server-computed debt summary (the errors/warnings/infos partition plus adopted/disabled rule counts), the real configuration problems, server-described Typeshed settings/status, and the two-layer caching state;
- `EditorMutation` adds allowlisted `SetTypeshedSetting` / `RemoveTypeshedSetting` and `SetCacheSetting` / `RemoveCacheSetting` to the four rule/tag variants;
- `TypeshedAction` is the closed `DownloadLatest` / `DownloadPinned` / `ViewLicense` union;
- a preview is the resolved per-rule effective-severity changes (`Disabled` = does not run, never present on a `pep` rule), the exact persisted Typeshed and cache setting changes, plus a complete errors/warnings/infos before/after partition; and
- occurrences are paged locations with the severity that produced them.

There is no format enum, no preset list, and no mutation intents. The debt summary, path inventory, configuration problems, and adoption state a view renders are read-only server projections of the snapshot — never client-computed and never a new kind of config line.

## Configuration sources and writes {#CONFIGEDITOR-SOURCES}

For one root, the editor targets the root's `pyproject.toml` — the existing file when present, otherwise the seeding target ([LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING)). A legacy `basilisk.json` is never read or written. Every mutation targets the one active document.

The writer validates the original structure, validates every requested mutation, renders the complete replacement, and validates it again before returning a patch. TOML edits preserve unrelated content, comments, ordering, and newline style. Removing every entry leaves explicitly empty tables: an empty table means `analyze` runs nothing — a legitimate user choice — and pruning it would re-arm the one-time seed, so empty tables are never pruned. Parent tables the writer itself creates only as a path to a deeper mutation target stay implicit — the render never introduces a bare `[tool]`-style header with no entries of its own, exactly as if the dotted headers had been written by hand; a table that exists in the user's document keeps its explicit header, however empty.

Closed-source apply sends a whole-document `WorkspaceEdit`, then keeps a root-scoped in-memory overlay until the client write is visible on disk. Disk revision checks prevent a stale preview from overwriting an external edit. The client owns making the write durable: after a successful apply the VS Code client saves the configuration document its edit dirtied, while a source that already carried the user's own unsaved edits stays under the user's control and is not saved implicitly.

### Open buffers and optimistic locks {#CONFIGEDITOR-SOURCES-OPEN-BUFFER}

Clients synchronize candidate `pyproject.toml` documents in addition to Python. The LSP accepts both the root-level document and nested folder-config documents into its configuration state — nested candidates participate as folder overrides ([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)) and are never analysed as Python. `didOpen`, incremental `didChange`, `didSave`, and `didClose` keep that state aligned with the editor.

When the active source is open, its in-memory text is authoritative for snapshot, preview, validation, and apply, even if the disk source is malformed. Apply rechecks the content revision and emits a `TextDocumentEdit` carrying the current LSP document version. A processed content change fails the revision check; a change racing the client edit fails the versioned workspace edit. A short-lived pending projection bridges successful `workspace/applyEdit` and its following `didChange` without changing the base text used for incremental edits. Closing the buffer removes the projection, restores disk authority, and runs the shared refresh tail from the on-disk content ([LSPARCH-CONFIG](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG)): discarding unsaved edits changes the effective configuration without touching disk, so no watcher fires — the close itself triggers the refresh.

## Suppression diagnostics {#CONFIGEDITOR-SUPPRESSIONS}

The four suppression-audit rules (`BSK-0060`–`BSK-0063`) are ordinary analyze-scope rules whose classification, emission order, and precedence are defined in [CHKARCH-STRICTNESS-SUPPRESSION-DIAGNOSTICS](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-STRICTNESS-SUPPRESSION-DIAGNOSTICS); the standard seed turns them on ([LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING)). In the editor they behave like any rule: they carry the `basilisk` and `suppressions` tags and participate in tag facets, occurrence pagination, and preview/apply.

## VS Code experience {#CONFIGEDITOR-VSIX-EXPERIENCE}

The capability-gated **Basilisk: Open Configuration Editor** command opens one full-width singleton webview using the shared Signals store and typed LSP transport. A left navigation rail selects among five views — **Overview**, **Rules**, **Adoption**, **Path Overrides**, and **Project** — and every view renders exact server-computed state; none is a synthetic score. Rules is the default view.

The **Overview** view is a read-only effective-state dashboard: the root's error/warning/info debt partition and total diagnostic count, and the counts of adopted (graded-down `pep`) and disabled rules — all folded by the server into the snapshot's debt summary, never computed on the client.

The Rules view is tag-first, grouping source, PEP-category, and policy facets. Tag groups expose the tag's entry control; rows expose per-rule entry controls. Every control lists concrete severities only — there is no separate "no entry" choice, because an analyze rule or tag with no entry already resolves to disabled ([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL) resolution step 3), so the two choices were one. A control with no underlying entry displays what no entry resolves to (the rule's effective severity; `error` for pep-affecting tags, `disabled` otherwise), and choosing a value always writes an explicit entry (`SetRule` / `SetTag` — an explicit `disabled` beats any tag entry, so Disabled always disables; `RemoveRule` / `RemoveTag` stay wire-only). `pep`-affecting controls — pep rows, the `pep` source tag, and PEP-category tags — offer `error` / `warning` / `info` with no disable control, because no disable exists for pep rules; analyze rows and non-pep tags additionally offer `disabled`. The view supports search, virtualized rows, exact impact preview/apply, paged occurrence navigation, and conflict refresh.

The **Adoption** view renders the server's effective adoption state read-only — the rules with open diagnostics and the remaining debt to pay down — and offers the **Adopt workspace debt** and **Apply safe fixes** actions. Those invoke the standalone adopt and mass-fix commands ([AUTOFIX-ADOPTION](LSP-MASS-AUTOFIX-SPEC.md#AUTOFIX-ADOPTION), [AUTOFIX](LSP-MASS-AUTOFIX-SPEC.md#AUTOFIX)); they are not configuration-editor mutations, and the view computes no debt of its own.

The **Path Overrides** view surfaces the nested `pyproject.toml` `[tool.basilisk]` tables the server discovered under the root ([CHKARCH-CONFIG-DISCOVERY](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-DISCOVERY)): each is shown with its folder, its rule and tag entries, and a link that opens that folder's configuration file for editing. The checker honors the nearest deciding table per rule; the view exposes only folder-scoped tables — glob-path and per-module override tables stay excluded.

The **Project** view shows the active configuration source resolved by the server — its `pyproject.toml` URI, whether it exists on disk, whether it is writable, and the content revision — plus any real configuration problems (an entry naming an unknown rule code), the Typeshed settings ([LSPCFGED-TYPESHED](#LSPCFGED-TYPESHED)), and the caching panel ([LSPCFGED-CACHE](#LSPCFGED-CACHE)). The extension never reads or writes configuration files itself.

Multi-root selection is explicit: the active editor's root wins, otherwise the user chooses a workspace. Responses and navigation are checked against that root. The extension does not read or write configuration files itself. The analyze opt-out is an ordinary editor setting relayed as an initialization option ([LSPARCH-DIAGNOSTIC-SCOPE](LSP-ARCHITECTURE-SPEC.md#LSPARCH-DIAGNOSTIC-SCOPE)), not part of this editor.

A non-PEP diagnostic's hover carries a **Configure Severity** deep link ([LSPARCH-FEATURES-HOVER](LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-HOVER)): the LSP embeds a `command:basilisk.openConfigurationEditor` link with a `{ "rule": <code> }` argument, the client trusts hover markdown for exactly that one command, and the opened editor focuses the rule once per webview lifetime — the search filter is prefilled with the code and the rule's detail opens. The argument is untrusted input: anything but a bounded, non-empty string is ignored, and an unknown code opens the editor unfocused. PEP rules get no link — no disable exists for them.

## Typeshed settings {#LSPCFGED-TYPESHED}

The standard-library source implements the pinned typing specification's custom
"canonical source" option
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
Freshness is the default; determinism is one control away. Every key in
[STUBRES-TYPESHED-CONFIG](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-CONFIG)
is editable here.

There are **two** sources and no third. The snapshot carries ONE active source
holding the value that defines it — `ExactCommit { commit }` or
`CustomFolder { path }` — so "a pin plus a custom folder" cannot be described at
all. Alongside it the server sends the store folder (absent for a custom
folder), and whether a license document exists to open. There are no per-control
widget, label, or enabled descriptors: copy is client presentation, availability
is the data itself.

| Source | Chosen by | Writes | Step-3 effect |
|---|---|---|---|
| **Pinned commit** *(default)* | editing the SHA, or **Download latest** | full `typeshed-commit`, clears `typeshed-path` | that SHA, verified offline; fails closed if it is not on this machine |
| **Custom folder** | selecting it (folder-picker) | `typeshed-path`, clears `typeshed-commit` | canonical user-managed tree |

Selecting a source is one atomic transition, and only the ACTIVE source's own
field is rendered. A SHA that is not 40 hexadecimal characters is refused in the
field and never reaches the configuration.

| Control | Key | Widget |
|---|---|---|
| Store folder | `typeshed-store-path` | folder-picker, under Advanced |
| License | active source | **View license**, or `not supplied` for custom |

A Typeshed edit has no rule-severity impact to weigh, so it is written as soon as
it is made — no impact dialog stands between the control and the configuration,
and every control re-renders from the snapshot that results. The impact dialog
remains for rule and tag entries; dismissing it discards the change and returns
every control to the configuration that still holds.

Only a pinned commit suppresses `typeshed_source_unpinned`; Custom says its folder can change and
should be versioned or content-addressed externally, and shows user-managed terms
rather than the typeshed composite license
([STUBRES-TYPESHED-WARN](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN)).
`ViewLicense` returns the active immutable license document, or `not supplied`
for custom. Clients execute nothing locally.

The two directory keys render with a native folder-picker rather than free text;
they are the only path-typed settings the editor exposes, distinct from the
glob-path and per-module rule overrides it deliberately excludes
([§CONFIGEDITOR-ACCEPTANCE](#CONFIGEDITOR-ACCEPTANCE)).
Folder selection feeds the ordinary validated transaction in
[§CONFIGEDITOR-SOURCES](#CONFIGEDITOR-SOURCES); cancellation writes nothing and
restores the controls to the active source.

### Download {#LSPCFGED-TYPESHED-DOWNLOAD}

Downloading is **not configuration**: it is a button, backed by a component that
lives outside the editor
([STUBRES-TYPESHED-DOWNLOAD](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-DOWNLOAD)).
The editor never downloads to satisfy an edit, and no configuration change ever
triggers one.

| Button | Offered when | Does |
|---|---|---|
| **Download latest** | always, except while a download is running | resolves `main`, acquires it, writes that SHA as `typeshed-commit` |
| **Download pinned** | the pinned commit is not on this machine | acquires exactly that SHA; writes no configuration |

A running download shows progress **on the button that started it**. Nothing
else is blocked: there is no full-panel overlay, no modal, and no lock screen —
every other control stays live, because reading and writing configuration never
waited on the network in the first place. Changing the pin re-resolves locally
and takes effect immediately.

### Service Info tree {#LSPCFGED-TYPESHED-SERVICE-INFO}

Service Info mirrors the server's active source/full SHA and composable warnings
from [STUBRES-TYPESHED-WARN](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN):

| State | Row |
|---|---|
| pin absent from this machine, or verification failed | persistent `NO SOURCE`; analysis does not run; no substitute source |
| no explicit commit (the bundled commit is serving) | persistent `typeshed_source_unpinned` |
| license drift | persistent `typeshed_source_license_changed`; activation blocked |
| custom | persistent `typeshed_source_unpinned` + `typeshed_source_user_managed` |
| download running | spinner, on that action only |

Rows may coexist, never poll, and never mutate; fixes remain in the editable
section. A warning row's message names its fix (e.g. `NO SOURCE`'s **Download
pinned**), so the row carries exactly one navigation-only command that opens
the configuration editor — attached only while the server is running and
advertises the editor capability, because the open command is gated on that
same pair and a shown-but-dead command is forbidden. Other rows carry no
command. They describe Basilisk transport around pinned typing step 3, not extra
typing diagnostics
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

## Caching {#LSPCFGED-CACHE}

Basilisk caches on **two layers**, and this panel names both. A configuration
surface that showed only the switchable one would read as *"this is all the
caching there is"* — which is precisely the question the panel exists to
answer. Both layers are described in
[CHKCACHE-CONFIG-SALSA](CHECKER-CACHE-SPEC.md#CHKCACHE-CONFIG-SALSA).

| Layer | Lifetime | Rendered as |
|---|---|---|
| **Persistent result cache** ([CHKCACHE](CHECKER-CACHE-SPEC.md#CHKCACHE)) | across processes, on disk | editable controls |
| **In-session Salsa memoization** ([CHKARCH-INCREMENTAL-SALSA](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA)) | one session, in memory | read-only rows |

Every key in [CHKCACHE-CONFIG](CHECKER-CACHE-SPEC.md#CHKCACHE-CONFIG) is
editable here, and nothing else is:

| Control | Key | Widget |
|---|---|---|
| Reuse results between runs | `cache` | checkbox; always writes an explicit `true`/`false` |
| Cache folder | `cache-dir` | folder-picker, with **Use default folder** — which REMOVES the key rather than writing the default back as an entry |

The checkbox writes an explicit value for the same reason a severity control
does ([§CONFIGEDITOR-VSIX-EXPERIENCE](#CONFIGEDITOR-VSIX-EXPERIENCE)): what the
panel shows is then what the file says, with no inferred middle state. The
folder field always shows the **effective** location — the configured folder or
the default — resolved server-side through the one resolver the CLI writes
entries through (`CHKCACHE-CONFIG-ONE-RESOLVER`), so the panel can never
display a folder the run would not use; the reset control appears only once
there is a project choice to undo.

The in-session rows are read-only and carry **no control of any kind**, because
the Salsa layer has no key: it reports the engine, that it is always on, and
the live count of files the session currently memoizes. That count is the only
evidence a reader has that the layer is running at all.

Like a Typeshed edit, a caching edit has no rule-severity impact to weigh, so
it is written as soon as it is made — the impact dialog never stands between
these controls and the configuration
([§LSPCFGED-TYPESHED](#LSPCFGED-TYPESHED)). Cancelling the folder picker writes
nothing and restores the controls to the configuration that still holds.

## Accessibility and security {#CONFIGEDITOR-ACCESSIBILITY-SECURITY}

The webview uses theme tokens, text-labelled severities, keyboard controls, high-contrast/responsive styles, reduced-motion handling, a default-deny CSP, nonce-gated local scripts, no remote resources, and runtime-decoded intents. Workspace data arrives only after the ready handshake and is never interpolated into executable HTML.

Automated tests cover the CSP/data boundary, intent decoding, semantic labels, responsive/reduced-motion styles, stale async result rejection, singleton message binding, capability gating, and typed routing. Recorded cross-platform keyboard, screen-reader, zoom, CSP, and injection audits remain release gates.

## Acceptance {#CONFIGEDITOR-ACCEPTANCE}

The acceptance surface is: the five LSP operations over rule/tag entries and allowlisted Typeshed and caching settings/actions, the five-view VS Code client (Overview, Rules, Adoption, Path Overrides, Project) rendering only server-computed state, and unsaved-buffer/apply-race safety. The partition, seeding, and diagnostic scope are accepted where they are specified ([CHKARCH-COMMANDS](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-COMMANDS), [LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING), [LSPARCH-DIAGNOSTIC-SCOPE](LSP-ARCHITECTURE-SPEC.md#LSPARCH-DIAGNOSTIC-SCOPE)).

The contract deliberately excludes — and tests assert the absence of — selector-based mutations, `Inherit`/`Native` intents, native/default severities, presets, mutation intents and rule-family booleans, glob path and per-module override tables (folder configs cover scoped grading and back the Path Overrides view), fixability selectors and per-occurrence fix-safety metadata (mass fixes and adopt/un-adopt are the standalone [AUTOFIX](LSP-MASS-AUTOFIX-SPEC.md#AUTOFIX) commands), configuration-format enums, shadowed-source reporting, and any control over the in-session Salsa layer (it has no key — [LSPCFGED-CACHE](#LSPCFGED-CACHE) reports it, never edits it). Malformed configuration surfaces as a structured request error, distinct from the real configuration-problem inventory the Project view renders.
