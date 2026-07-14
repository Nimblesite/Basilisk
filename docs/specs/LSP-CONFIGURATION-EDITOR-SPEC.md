# Configuration editor {#CONFIGEDITOR}

This contract composes:

- severity and precedence from [CHKARCH-STRICTNESS](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-STRICTNESS);
- canonical rule tags from [CHKTAG](CHECKER-RULE-TAGGING-SPEC.md#CHKTAG);
- shared methods from [LSPARCH-CONFIG-EDITOR](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-EDITOR);
- fixes and adoption from [AUTOFIX](LSP-MASS-AUTOFIX-SPEC.md#AUTOFIX); and
- VS Code hosting from [VSIX-CONFIGURATION-EDITOR](VSIX-SPEC.md#VSIX-CONFIGURATION-EDITOR).

The LSP owns rule selection, configuration parsing, impact analysis, and mutation. Clients render server data, request server operations, and apply the returned edit. They do not maintain another rule catalog or write project configuration directly.

## Rule catalog and tags {#CONFIGEDITOR-TAGS}

The checker registry supplies each rule's code, title, summary, documentation URL, native severity, default-enabled state, and canonical tags. The LSP exposes the same catalog with tag kinds, rule counts, current diagnostic counts, and any/all tag selectors.

Tags are the primary information architecture. Provenance, reserved PEP category, and descriptive policy tags are flat facets, and one rule may belong to several facets. The client groups those facets for navigation without inventing a hierarchy or copying tag membership.

Fixability is currently projected by the LSP from the mass-fix safe/all rule lists rather than stored in the canonical checker catalog. Consolidating that metadata remains tracked in [CONFIGEDITOR-PLAN-DOMAIN](../plans/LSP-CONFIGURATION-EDITOR-PLAN.md#CONFIGEDITOR-PLAN-DOMAIN).

## Severity semantics {#CONFIGEDITOR-SEVERITY}

Every rule is independently configurable at project or path scope:

| Editor intent | Persisted result |
|---|---|
| Inherited | Remove the explicit override |
| Native | Write the selected rule's concrete native severity |
| Error / Warning / Info | Enable the rule at that severity |
| Disabled | Persist an explicit disabled severity |

An explicit non-disabled severity enables an otherwise opt-in Basilisk rule. Removing it returns selection to the rule's `defaultEnabled` state; tags are navigation and bulk-selection metadata, never ambient switches. `Inherited` and `Native` are mutation intents, not values stored in the config file.

When several path patterns match, the checker selects one winner deterministically: a non-wildcard pattern outranks a wildcard pattern, then more path segments outrank fewer, then more literal characters outrank fewer, with a stable lexical tie-break. That winning path entry takes precedence over the project entry as specified by [CHKARCH-STRICTNESS-PRECEDENCE](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-STRICTNESS-PRECEDENCE).

## Presets {#CONFIGEDITOR-PRESETS}

Snapshots advertise reusable, one-shot mutation recipes:

- **Strict:** every live rule at its native severity;
- **Maximum:** every live rule at error severity; and
- **Suppression audit:** the `suppressions` tag at native severity.

The client previews and applies the advertised ordinary mutations. `All` and `Native` expand against the live catalog, so Strict writes an explicit severity for every rule, including opt-in rules. Preset IDs are never written to project configuration; the resulting explicit rule entries are the complete durable state.

## LSP operations {#CONFIGEDITOR-OPERATIONS}

All configuration-editor requests require an explicit active-workspace `rootUri`:

- `basilisk/configurationSnapshot` returns the active source and revision, shadowed sources, live catalog, tag facets, current debt, presets, and a normalized inventory of every persisted path override.
- `basilisk/previewConfigurationChange` validates the base revision, expands all/code/tag/current-violation/safe-fixability selectors, builds a validated in-memory patch, and reruns checking against that hypothetical config.
- `basilisk/applyConfigurationChange` consumes a cached preview with the same root and revision, asks the client to apply one configuration edit, reloads and rechecks the root, republishes diagnostics, emits `basilisk/configurationChanged`, and returns a fresh snapshot.
- `basilisk/ruleOccurrences` returns URI/range/code-ordered pages. The opaque cursor resumes the stable result and the server accepts limits from 1 to 1000.

Snapshot, preview, and occurrence inventory covers the complete selected root, even when analysis is configured to publish only open files. Open buffers stay authoritative; eligible closed files are loaded from disk into the server index without publishing additional diagnostics. Preview impact therefore describes the root inventory, not only the currently visible editor tabs.

Safe source edits remain a standalone reusable LSP command: `basilisk.fixWorkspace` accepts an optional `{ "rootUri": "file:///..." }` argument. The configuration editor always supplies it, and the server validates and restricts edits to that exact active root. The no-argument command retains its existing all-indexed-roots behavior for older clients.

The legacy `basilisk.disableRule` command uses the same validated, root-aware configuration mutation service. Active configuration watchers also run the shared refresh tail and emit the changed notification.

Unknown roots, rule codes, tags, invalid selectors, severities, and path patterns, stale revisions, malformed configuration, expired previews, and client-rejected edits are request errors. Unrelated unknown configuration fields are currently accepted. `ConfigurationProblem` exists in the wire model, but v1 returns malformed configuration as a structured request error rather than populating snapshot or preview `problems`.

## Wire model {#CONFIGEDITOR-MODEL}

The design source is [`models/configuration_editor.td`](../../models/configuration_editor.td), with its rendered [SVG](../models/configuration_editor.svg). The committed Rust and TypeScript DTOs carry generated provenance and preserve those shapes plus their transport derives. An automated regeneration/drift check remains open.

A snapshot contains:

- root, content revision, active source, format (a single `PyprojectToml` variant; the wire enum keeps a deprecated, never-emitted `BasiliskJson` variant for protocol-v1 compatibility), existence/read-only state, and shadowed sources (ignored legacy `basilisk.json` files, reported but never read);
- rule descriptors with project-configured and effective severity plus diagnostic, file, fix, and adoption counts;
- typed tag facets, debt totals, and server-owned preset mutations; and
- sorted path entries with legacy disabled-list values normalized to `Disabled`, exact rule severities, and adoption provenance.

Configured severity on a rule row is currently the project-level projection. The path inventory is exact, but the snapshot does not yet expose the winning path or field-level provenance for an arbitrary source file.

## Configuration sources and writes {#CONFIGEDITOR-SOURCES}

For one root, discovery always selects the root's `pyproject.toml` — the existing file when present, otherwise `pyproject.toml` as the creation target. An existing root-level `basilisk.json` is surfaced in `shadowedSources` and is never read or written. Every mutation targets the one active document.

The writer validates the original structure, validates every requested severity, renders the complete replacement, and validates it again before returning a patch. TOML edits preserve unrelated content, comments, ordering, and newline style. Reset removes empty generated rule/path/adoption tables.

Closed-source apply sends a whole-document `WorkspaceEdit`, then keeps a root-scoped in-memory overlay until the client write is visible on disk. Disk revision checks prevent a stale preview from overwriting an external edit. The client owns making that write durable: applying a `WorkspaceEdit` only rewrites the in-memory buffer, so after a successful apply the VS Code client saves the configuration document its edit dirtied. A source that already carried the user's own unsaved edits stays under the user's control and is not saved implicitly.

### Open buffers and optimistic locks {#CONFIGEDITOR-SOURCES-OPEN-BUFFER}

Clients synchronize candidate `pyproject.toml` documents in addition to Python. The LSP accepts only exact root-level candidates into its configuration state; nested candidates are ignored and are never analysed as Python. `didOpen`, incremental `didChange`, `didSave`, and `didClose` keep that state aligned with the editor.

When the active source is open, its in-memory text is authoritative for snapshot, preview, validation, and apply, even if the disk source is malformed. Apply rechecks the content revision and emits a `TextDocumentEdit` carrying the current LSP document version. A processed content change fails the revision check; a change racing the client edit fails the versioned workspace edit. A short-lived pending projection bridges successful `workspace/applyEdit` and its following `didChange` without changing the base text used for incremental edits. Closing the buffer removes the projection and restores disk authority.

Rule policy, path exceptions, and adoption provenance all live in this active project config file. Presets introduce no second persisted state.

## Path overrides {#CONFIGEDITOR-PATHS}

The snapshot's `pathOverrides` inventory is sorted by pattern and contains every persisted rule severity for each path scope. A client can preview a new bounded exception or reset every listed rule in an existing entry without parsing the underlying TOML. Preview returns the exact normalized code/scope/resulting setting changes and reruns the full root before anything is written.

Path mutations use project-relative glob syntax. Adoption uses the same domain with exact project-relative file paths, so ordinary checker precedence and the configuration editor cannot disagree about what a path entry means.

## Strict-first workflow and adoption {#CONFIGEDITOR-ADOPTION}

The VS Code editor exposes the strict-first workflow as three explicit, server-owned operations:

1. Preview and apply Strict or Maximum, materializing the target as ordinary project rule severities.
2. Run root-scoped `basilisk.fixWorkspace`, which applies only the currently supported safe fixes, then refresh the snapshot.
3. Review paged `WithoutSafeFix` occurrences and preview an explicit severity change. The supplied project action uses `Disabled` and warns that it hides current and future diagnostics; users can instead choose a narrower path or a non-disabled severity.

The safe-fix edit and config preview are deliberately sequential operations, not one atomic request: current counts are reloaded after fixes before the debt selector is expanded.

`basilisk.adoptFile` and `basilisk.adoptWorkspace` provide the durable per-file alternative. They collect current error and safety-violation codes and persist warning-severity exact-file entries with `adoption = true` through the same active-config transaction. New files do not inherit those exceptions. `basilisk.unadoptFile` removes the generated rules for one file.

After an adopted file is saved, the LSP rechecks it and removes each adopted rule whose last matching diagnostic is gone. The structure-aware writer removes the empty exact-file entry, the normal refresh path republishes diagnostics, and `basilisk/configurationChanged` refreshes clients. The CLI and direct adoption commands do not run safe fixes themselves; the configuration editor's root-scoped fix action is the explicit first step when that workflow is wanted.

## Suppression diagnostics {#CONFIGEDITOR-SUPPRESSIONS}

The opt-in `suppressions` family emits audit diagnostics after ordinary inline suppression, so a directive cannot hide its own audit:

| Rule | Native severity | Meaning |
|---|---|---|
| `BSK-I0060` | Info | Active code-specific directive |
| `BSK-W0061` | Warning | Active blanket directive |
| `BSK-W0062` | Warning | Unused directive |
| `BSK-E0063` | Error | Malformed, unknown, conflicting, or unpaired directive |

All four rules are disabled by default, carry `basilisk` and `suppressions`, and use the same project/path severity configuration as every other rule. The Suppression audit preset enables them at native severity; users can promote any of them to error or demote them independently. They participate in root debt, tag selection, occurrence pagination, and preview/apply.

Malformed or misplaced directives are audited but never applied to ordinary diagnostics. Focused tests cover the off-by-default gate, all four configured severities, self-suppression, line/block/file spans, malformed boundaries and spellings, and standard PEP 484 type-comment exclusion. Stable directive IDs, raw spelling, and diagnostic-origin metadata are not yet exposed on the LSP wire.

## VS Code experience {#CONFIGEDITOR-VSIX-EXPERIENCE}

The capability-gated **Basilisk: Open Configuration Editor** command opens one full-width singleton webview. It uses the shared Signals store and typed LSP transport, with Overview, Rules, Adoption, Path Overrides, and Project views.

The Rules view is tag-first, grouping source, PEP-category, and policy facets. It supports search, virtualized rows, individual and bulk severity/reset controls, server-advertised presets, exact impact preview/apply, paged occurrence navigation, and conflict refresh. The Adoption view delegates safe fixes to the root-scoped LSP command and delegates remaining-debt selection to `WithoutSafeFix`. The Path view renders the server inventory and previews all changes through the same transaction API.

Multi-root selection is explicit: the active editor's root wins, otherwise the user chooses a workspace. Responses and navigation are checked against that root. The extension does not read or write configuration files itself.

## Accessibility and security {#CONFIGEDITOR-ACCESSIBILITY-SECURITY}

The webview uses theme tokens, text-labelled severities, keyboard controls, high-contrast/responsive styles, reduced-motion handling, a default-deny CSP, nonce-gated local scripts, no remote resources, and runtime-decoded intents. Workspace data arrives only after the ready handshake and is never interpolated into executable HTML.

Automated tests cover the CSP/data boundary, intent decoding, semantic labels, responsive/reduced-motion styles, stale async result rejection, singleton message binding, capability gating, and typed routing. The committed headed capture exercises the tag-first rules view against the real LSP. Recorded cross-platform keyboard, screen-reader, zoom, CSP, and injection audits remain release gates.

## Acceptance and follow-up {#CONFIGEDITOR-ACCEPTANCE}

The v1 acceptance surface is the config-only domain and LSP operations above, the thin tag-first VS Code client, unsaved-buffer/apply-race safety, the real-LSP screenshot, and a clean repository CI run. Neovim/Zed clients, generated DTO drift checks, canonical fixability metadata, richer provenance/problem records, rich suppression metadata, and broader transaction/accessibility matrices are tracked as non-blocking follow-up work. They extend the v1 contract; they do not introduce policy modes or client-owned configuration state.
