# Configuration editor {#CONFIGEDITOR}

This contract composes:

- the configuration model and command partition from [CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL) and [CHKARCH-COMMANDS](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-COMMANDS), with discovery from [CHKARCH-CONFIG-DISCOVERY](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-DISCOVERY);
- LSP seeding from [LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING) and diagnostic scope from [LSPARCH-DIAGNOSTIC-SCOPE](LSP-ARCHITECTURE-SPEC.md#LSPARCH-DIAGNOSTIC-SCOPE);
- canonical rule tags from [CHKTAG](CHECKER-RULE-TAGGING-SPEC.md#CHKTAG);
- shared methods from [LSPARCH-CONFIG-EDITOR](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-EDITOR); and
- VS Code hosting from [VSIX-CONFIGURATION-EDITOR](VSIX-SPEC.md#VSIX-CONFIGURATION-EDITOR).

The LSP owns the rule catalog, configuration parsing, impact analysis, and mutation. Clients render server data, request server operations, and apply the returned edit. They do not maintain another rule catalog or write project configuration directly.

The editor is a veneer over the configuration model, never an extension of it. A config file can express exactly two kinds of line — a rule entry and a tag entry — so the editor can request exactly four things: set or remove one of each. Every other editor concept (snapshots, previews, occurrence pages) exists only on the wire.

## Rule catalog and tags {#CONFIGEDITOR-TAGS}

The checker registry supplies each rule's code, title, summary, documentation URL, and canonical tags ([CHKTAG](CHECKER-RULE-TAGGING-SPEC.md#CHKTAG)). The LSP exposes that catalog with each rule's per-rule entry (if any), its resolved effective severity, and its current diagnostic count.

Tags serve two roles, both explicit. As *facets*, they group the catalog for navigation — provenance (`pep`, `basilisk`), reserved PEP category, and descriptive policy tags are flat facets and one rule may carry several. As *entries*, a tag written into `[tool.basilisk.rule-tags]` grades every rule carrying it — one visible line of config, never an implicit switch. The snapshot reports each tag's entry alongside its facet data.

## LSP operations {#CONFIGEDITOR-OPERATIONS}

All configuration-editor requests require an explicit active-workspace `rootUri`:

- `basilisk/configurationSnapshot` returns the root config document URI, its content revision, the catalog with per-rule entries, effective severities, and diagnostic counts, and tag states with their entries.
- `basilisk/previewConfigurationChange` validates the base revision, builds a validated in-memory patch from the requested mutations, and reruns checking against that hypothetical config, returning the resolved per-rule effective-severity changes and before/after impact.
- `basilisk/applyConfigurationChange` consumes a cached preview identified by root and preview ID; the server rejects it if the preview's pinned base revision no longer matches the current document. It asks the client to apply one configuration edit, reloads and rechecks the root, republishes diagnostics, emits `basilisk/configurationChanged`, and returns a fresh snapshot.
- `basilisk/ruleOccurrences` returns URI/range/code-ordered pages selected by the all/codes/tags selectors. The opaque cursor resumes the stable result and the server accepts limits from 1 to 1000.

A mutation is `SetRule`, `RemoveRule`, `SetTag`, or `RemoveTag` — nothing else. Requesting `disabled` for a `pep`-tagged rule (directly, or via a tag entry that would resolve one to `disabled`) is a request error: PEP rules are graded, never disabled ([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)).

Snapshot, preview, and occurrence inventory cover the complete selected root, even when analysis is configured to publish only open files. Open buffers stay authoritative; eligible closed files are loaded from disk into the server index without publishing additional diagnostics.

The `basilisk.disableRule` command writes an explicit `disabled` rule entry through the same validated, root-aware mutation service, and is rejected for `pep`-tagged rules. Configuration watching is server-owned ([LSPARCH-CONFIG](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG)).

Unknown roots, rule codes, tags, severities, and selectors, pep-disable requests, stale revisions, malformed configuration, expired previews, and client-rejected edits are request errors ([LSPARCH-CONFIG-EDITOR-ERRORS](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-EDITOR-ERRORS)).

## Wire model {#CONFIGEDITOR-MODEL}

The editor protocol's design source is [`models/configuration_editor.td`](../../models/configuration_editor.td). It builds on the core configuration model in [`models/configuration.td`](../../models/configuration.td) ([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)) — `RuleCode`, `RuleTag`, `RuleSeverity` — and **MUST NOT add anything to it**: the two models are separate files precisely so editor machinery can never contaminate the config model. Language DTOs are regenerated from the two files together.

The protocol is deliberately small:

- a snapshot is the root, the config document URI, a content revision, rule states (descriptor + optional entry + effective severity + diagnostic count), and tag states (facet + optional entry + counts);
- an `EditorMutation` is `SetRule` / `RemoveRule` / `SetTag` / `RemoveTag`;
- a preview is the resolved per-rule effective-severity changes (`Disabled` = does not run, never present on a `pep` rule) plus a complete errors/warnings/infos before/after partition; and
- occurrences are paged locations with the severity that produced them.

There is no format enum, no problems list, no debt summary, no preset list, no path inventory, and no provenance flags.

## Configuration sources and writes {#CONFIGEDITOR-SOURCES}

For one root, the editor targets the root's `pyproject.toml` — the existing file when present, otherwise the seeding target ([LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING)). A legacy `basilisk.json` is never read or written. Every mutation targets the one active document.

The writer validates the original structure, validates every requested mutation, renders the complete replacement, and validates it again before returning a patch. TOML edits preserve unrelated content, comments, ordering, and newline style. Removing every entry leaves explicitly empty tables: an empty table means `analyze` runs nothing — a legitimate user choice — and pruning it would re-arm the one-time seed, so empty tables are never pruned.

Closed-source apply sends a whole-document `WorkspaceEdit`, then keeps a root-scoped in-memory overlay until the client write is visible on disk. Disk revision checks prevent a stale preview from overwriting an external edit. The client owns making the write durable: after a successful apply the VS Code client saves the configuration document its edit dirtied, while a source that already carried the user's own unsaved edits stays under the user's control and is not saved implicitly.

### Open buffers and optimistic locks {#CONFIGEDITOR-SOURCES-OPEN-BUFFER}

Clients synchronize candidate `pyproject.toml` documents in addition to Python. The LSP accepts both the root-level document and nested folder-config documents into its configuration state — nested candidates participate as folder overrides ([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)) and are never analysed as Python. `didOpen`, incremental `didChange`, `didSave`, and `didClose` keep that state aligned with the editor.

When the active source is open, its in-memory text is authoritative for snapshot, preview, validation, and apply, even if the disk source is malformed. Apply rechecks the content revision and emits a `TextDocumentEdit` carrying the current LSP document version. A processed content change fails the revision check; a change racing the client edit fails the versioned workspace edit. A short-lived pending projection bridges successful `workspace/applyEdit` and its following `didChange` without changing the base text used for incremental edits. Closing the buffer removes the projection, restores disk authority, and runs the shared refresh tail from the on-disk content ([LSPARCH-CONFIG](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG)): discarding unsaved edits changes the effective configuration without touching disk, so no watcher fires — the close itself triggers the refresh.

## Suppression diagnostics {#CONFIGEDITOR-SUPPRESSIONS}

The four suppression-audit rules (`BSK-0060`–`BSK-0063`) are ordinary analyze-scope rules whose classification, emission order, and precedence are defined in [CHKARCH-STRICTNESS-SUPPRESSION-DIAGNOSTICS](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-STRICTNESS-SUPPRESSION-DIAGNOSTICS); the standard seed turns them on ([LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING)). In the editor they behave like any rule: they carry the `basilisk` and `suppressions` tags and participate in tag facets, occurrence pagination, and preview/apply.

## VS Code experience {#CONFIGEDITOR-VSIX-EXPERIENCE}

The capability-gated **Basilisk: Open Configuration Editor** command opens one full-width singleton webview using the shared Signals store and typed LSP transport.

The Rules view is tag-first, grouping source, PEP-category, and policy facets. Tag groups expose the tag's entry control (set/remove a `rule-tags` severity); rows expose per-rule entry controls. `pep`-tagged rows offer `error` / `warning` / `info` / remove entry — no disable control, because no disable exists for them; analyze rows additionally offer `disabled`. The view supports search, virtualized rows, exact impact preview/apply, paged occurrence navigation, and conflict refresh.

Multi-root selection is explicit: the active editor's root wins, otherwise the user chooses a workspace. Responses and navigation are checked against that root. The extension does not read or write configuration files itself. The analyze opt-out is an ordinary editor setting relayed as an initialization option ([LSPARCH-DIAGNOSTIC-SCOPE](LSP-ARCHITECTURE-SPEC.md#LSPARCH-DIAGNOSTIC-SCOPE)), not part of this editor.

## Accessibility and security {#CONFIGEDITOR-ACCESSIBILITY-SECURITY}

The webview uses theme tokens, text-labelled severities, keyboard controls, high-contrast/responsive styles, reduced-motion handling, a default-deny CSP, nonce-gated local scripts, no remote resources, and runtime-decoded intents. Workspace data arrives only after the ready handshake and is never interpolated into executable HTML.

Automated tests cover the CSP/data boundary, intent decoding, semantic labels, responsive/reduced-motion styles, stale async result rejection, singleton message binding, capability gating, and typed routing. Recorded cross-platform keyboard, screen-reader, zoom, CSP, and injection audits remain release gates.

## Acceptance {#CONFIGEDITOR-ACCEPTANCE}

The acceptance surface is: the four LSP operations over the core configuration model (rule entries + tag entries, pep-disable rejected), the thin tag-first VS Code client, and unsaved-buffer/apply-race safety. The partition, seeding, and diagnostic scope are accepted where they are specified ([CHKARCH-COMMANDS](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-COMMANDS), [LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING), [LSPARCH-DIAGNOSTIC-SCOPE](LSP-ARCHITECTURE-SPEC.md#LSPARCH-DIAGNOSTIC-SCOPE)).

The contract deliberately excludes — and tests assert the absence of — selector-based mutations, `Inherit`/`Native` intents, native/default severities, presets, glob path and per-module overrides (folder configs cover scoped grading), per-file adoption state, fixability selectors and per-occurrence fix-safety metadata (mass fixes are the standalone [AUTOFIX](LSP-MASS-AUTOFIX-SPEC.md#AUTOFIX) command), configuration-format enums, shadowed-source reporting, and problem lists (malformed configuration is a structured request error).
