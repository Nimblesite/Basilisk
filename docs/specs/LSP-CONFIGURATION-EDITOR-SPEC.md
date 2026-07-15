# Configuration editor {#CONFIGEDITOR}

This contract composes:

- the configuration model from [CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL) and per-file discovery from [CHKARCH-CONFIG-DISCOVERY](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-DISCOVERY);
- LSP seeding from [LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING);
- canonical rule tags from [CHKTAG](CHECKER-RULE-TAGGING-SPEC.md#CHKTAG);
- shared methods from [LSPARCH-CONFIG-EDITOR](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-EDITOR); and
- VS Code hosting from [VSIX-CONFIGURATION-EDITOR](VSIX-SPEC.md#VSIX-CONFIGURATION-EDITOR).

The LSP owns the rule catalog, configuration parsing, impact analysis, and mutation. Clients render server data, request server operations, and apply the returned edit. They do not maintain another rule catalog or write project configuration directly.

The editor is a veneer over the configuration model, never an extension of it. It reads and writes plain rule entries — code plus severity — and every editor concept (selectors, snapshots, previews) exists only on the wire. Nothing the editor does introduces state the config file cannot express by itself.

## Rule catalog and tags {#CONFIGEDITOR-TAGS}

The checker registry supplies each rule's code, title, summary, documentation URL, and canonical tags ([CHKTAG](CHECKER-RULE-TAGGING-SPEC.md#CHKTAG)). The LSP exposes that catalog with each rule's current config entry (if any) and current diagnostic count.

Tags are navigation and bulk-selection metadata, never switches. Provenance (`pep`, `basilisk`), reserved PEP category, and descriptive policy tags are flat facets, and one rule may belong to several. The client groups those facets for browsing and bulk mutation; selecting a tag expands, server-side at preview time, to plain per-rule entries.

## LSP operations {#CONFIGEDITOR-OPERATIONS}

All configuration-editor requests require an explicit active-workspace `rootUri`:

- `basilisk/configurationSnapshot` returns the root config document URI, its content revision, the catalog with each rule's entry and diagnostic count, and tag facets.
- `basilisk/previewConfigurationChange` validates the base revision, expands the all/codes/tags selectors to plain per-rule entry changes, builds a validated in-memory patch, and reruns checking against that hypothetical config, returning the resolved entry changes and before/after diagnostic impact.
- `basilisk/applyConfigurationChange` consumes a cached preview identified by root and preview ID; the server rejects it if the preview's pinned base revision no longer matches the current document. It asks the client to apply one configuration edit, reloads and rechecks the root, republishes diagnostics, emits `basilisk/configurationChanged`, and returns a fresh snapshot.
- `basilisk/ruleOccurrences` returns URI/range/code-ordered pages. The opaque cursor resumes the stable result and the server accepts limits from 1 to 1000.

Snapshot, preview, and occurrence inventory cover the complete selected root, even when analysis is configured to publish only open files. Open buffers stay authoritative; eligible closed files are loaded from disk into the server index without publishing additional diagnostics.

The legacy `basilisk.disableRule` command writes an explicit `disabled` entry through the same validated, root-aware mutation service. Active configuration watchers run the shared refresh tail and emit the changed notification.

Unknown roots, rule codes, tags, severities, and selectors, stale revisions, malformed configuration, expired previews, and client-rejected edits are request errors ([LSPARCH-CONFIG-EDITOR-ERRORS](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-EDITOR-ERRORS)).

## Wire model {#CONFIGEDITOR-MODEL}

The editor protocol's design source is [`models/configuration_editor.td`](../../models/configuration_editor.td), with its rendered [SVG](../models/configuration_editor.svg). It builds on the core configuration model in [`models/configuration.td`](../../models/configuration.td) ([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)) — `RuleCode` and `RuleSeverity` — and **MUST NOT add anything to it**: the two models are separate files precisely so editor machinery can never contaminate the config model. Language DTOs are regenerated from the two files together; an automated drift check remains open.

The protocol is deliberately small:

- a snapshot is the root, the config document URI, a content revision, rule states (descriptor + optional entry + diagnostic count), and tag facets;
- an `EditorMutation` is `Set { selector, severity }` or `Remove { selector }` — the only two requests, matching the only two things a config file can express: an entry, or its absence;
- a preview is the resolved per-rule entry changes (`Option` on both sides — `None` means "no entry") plus diagnostic impact; and
- occurrences are paged locations with the severity that produced them.

There is no format enum, no problems list, no debt summary, no preset list, no path inventory, and no provenance flags.

## Configuration sources and writes {#CONFIGEDITOR-SOURCES}

For one root, the editor targets the root's `pyproject.toml` — the existing file when present, otherwise the seeding target ([LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING)). A legacy `basilisk.json` is never read or written. Every mutation targets the one active document.

The writer validates the original structure, validates every requested severity, renders the complete replacement, and validates it again before returning a patch. TOML edits preserve unrelated content, comments, ordering, and newline style. Removing every rule entry leaves an explicitly empty table: an empty table is meaningful (everything disabled) and is never pruned, because a missing table would re-trigger the in-memory PEP seed ([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)).

Closed-source apply sends a whole-document `WorkspaceEdit`, then keeps a root-scoped in-memory overlay until the client write is visible on disk. Disk revision checks prevent a stale preview from overwriting an external edit. The client owns making the write durable: after a successful apply the VS Code client saves the configuration document its edit dirtied, while a source that already carried the user's own unsaved edits stays under the user's control and is not saved implicitly.

### Open buffers and optimistic locks {#CONFIGEDITOR-SOURCES-OPEN-BUFFER}

Clients synchronize candidate `pyproject.toml` documents in addition to Python. The LSP accepts both the root-level document and nested folder-config documents into its configuration state — nested candidates participate as folder overrides ([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)) and are never analysed as Python. `didOpen`, incremental `didChange`, `didSave`, and `didClose` keep that state aligned with the editor.

When the active source is open, its in-memory text is authoritative for snapshot, preview, validation, and apply, even if the disk source is malformed. Apply rechecks the content revision and emits a `TextDocumentEdit` carrying the current LSP document version. A processed content change fails the revision check; a change racing the client edit fails the versioned workspace edit. A short-lived pending projection bridges successful `workspace/applyEdit` and its following `didChange` without changing the base text used for incremental edits. Closing the buffer removes the projection and restores disk authority.

## Suppression diagnostics {#CONFIGEDITOR-SUPPRESSIONS}

The suppression-audit family consists of four ordinary rules — they run only with an explicit config entry, and seeding writes them at `warning` like every other Basilisk rule ([LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING)):

| Rule | Meaning |
|---|---|
| `BSK-I0060` | Active code-specific directive |
| `BSK-W0061` | Active blanket directive |
| `BSK-W0062` | Unused directive |
| `BSK-E0063` | Malformed, unknown, conflicting, or unpaired directive |

Audit diagnostics are emitted after ordinary inline suppression is applied, so a directive cannot hide its own audit. Malformed or misplaced directives are audited but never applied to ordinary diagnostics. All four rules carry `basilisk` and `suppressions` tags and participate in tag selection, occurrence pagination, and preview/apply like any rule.

## VS Code experience {#CONFIGEDITOR-VSIX-EXPERIENCE}

The capability-gated **Basilisk: Open Configuration Editor** command opens one full-width singleton webview using the shared Signals store and typed LSP transport.

The Rules view is tag-first, grouping source, PEP-category, and policy facets. It supports search, virtualized rows, per-rule severity controls (`error` / `warning` / `info` / `disabled` / remove entry), bulk mutation by tag or selection, exact impact preview/apply, paged occurrence navigation, and conflict refresh.

Multi-root selection is explicit: the active editor's root wins, otherwise the user chooses a workspace. Responses and navigation are checked against that root. The extension does not read or write configuration files itself.

## Accessibility and security {#CONFIGEDITOR-ACCESSIBILITY-SECURITY}

The webview uses theme tokens, text-labelled severities, keyboard controls, high-contrast/responsive styles, reduced-motion handling, a default-deny CSP, nonce-gated local scripts, no remote resources, and runtime-decoded intents. Workspace data arrives only after the ready handshake and is never interpolated into executable HTML.

Automated tests cover the CSP/data boundary, intent decoding, semantic labels, responsive/reduced-motion styles, stale async result rejection, singleton message binding, capability gating, and typed routing. Recorded cross-platform keyboard, screen-reader, zoom, CSP, and injection audits remain release gates.

## Acceptance and removals {#CONFIGEDITOR-ACCEPTANCE}

The v1 acceptance surface is: the four LSP operations over the core configuration model, the thin tag-first VS Code client, and unsaved-buffer/apply-race safety. Seeding and the model itself are accepted where they are specified ([LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING), [CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)).

**Removed from this contract** (implementations still carrying them are legacy to remove):

- the fused single model file — the config model now lives in `models/configuration.td` and the editor wire protocol in `models/configuration_editor.td`, and the latter never contaminates the former;
- `Inherit` and `Native` mutation intents, `defaultSeverity`, `defaultEnabled`, and every notion of a rule's "native" severity;
- presets (`ConfigurationPreset`) and the strict-first preset workflow — seeding replaces them;
- glob path overrides (`MutationScope::Path`, `PathOverrideState`, path-precedence scoring) — folder configs replace them;
- per-file adoption in the editor contract (`adoption` provenance, adoption counts, debt summary);
- fixability selectors (`CurrentViolations`, `SafeFixable`, `WithoutSafeFix`) and per-occurrence fix-safety metadata — mass fixes remain a standalone command ([AUTOFIX](LSP-MASS-AUTOFIX-SPEC.md#AUTOFIX));
- `ConfigurationFormat` (including the deprecated `BasiliskJson` wire variant), shadowed-source reporting, and `ConfigurationProblem` — malformed configuration is a structured request error.

Neovim/Zed clients and the generated-DTO drift check remain non-blocking follow-up work.
