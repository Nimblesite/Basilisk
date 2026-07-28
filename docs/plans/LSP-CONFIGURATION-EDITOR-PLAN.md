# Configuration editor — release plan {#CONFIGEDITOR-PLAN}

> Contract: [LSP-CONFIGURATION-EDITOR-SPEC.md](../specs/LSP-CONFIGURATION-EDITOR-SPEC.md).

## Status {#CONFIGEDITOR-PLAN-STATUS}

The first complete VS Code slice is implemented: live catalog and tags,
per-rule severities, one-shot presets, active-config persistence, root-complete
preview/apply, path inventory, paged occurrences, the guided strict-first
workflow, suppression-audit rules, adoption graduation, and the tag-first
webview.

Checked items below have corresponding implementation and focused tests unless
the item explicitly says that only the wiring exists. The v1 release gate is a
clean full CI run. Other unchecked items are non-blocking follow-up work for
broader clients, richer metadata, and additional drift/audit automation.

## Configuration domain {#CONFIGEDITOR-PLAN-DOMAIN}

- [x] Derive the live rule catalog, native/default severities, and canonical
  tags from the checker registry rather than a VSIX-maintained list.
- [x] Persist `error`, `warning`, `info`, `disabled`, and inherited reset at
  project and path scope in the one active config document. (Originally
  `pyproject.toml` or `basilisk.json`; JSON support was later removed —
  `pyproject.toml` `[tool.basilisk]` is now the only source.)
- [x] Validate complete replacements, preserve TOML comments/newlines,
  normalize legacy disabled entries, and remove empty generated paths. (The
  JSON-alias preservation this item originally covered was removed along with
  `basilisk.json` support.)
- [x] Make overlapping path selection deterministic and cover exact-file over
  broad-glob precedence plus root-relative matching.
- [x] Expose a sorted, normalized path-override inventory with adoption
  provenance in every snapshot.
- [x] Persist the two persistent result-cache keys through the same validated
  transaction as rule/tag and Typeshed settings, and report BOTH caching layers
  in the snapshot — the configurable cross-session cache and the always-on,
  keyless in-session Salsa memoization ([LSPCFGED-CACHE], [CHKCACHE-CONFIG]).
- [ ] Put fixability metadata in the canonical checker catalog and make the LSP
  selectors plus website rule data consume it; remove the parallel safe/all
  fix lists.
- [ ] Add automated Rust/TypeScript DTO regeneration and a drift test for
  `models/configuration_editor.td`.
- [ ] Consolidate the analysis projection split between `BasiliskConfig` and
  LSP `WorkspaceConfig`.
- [ ] Add field/winning-path provenance and structured configuration problems;
  decide and enforce the policy for unrelated unknown fields.
- [x] Resolved by removal rather than a migration flow: `basilisk.json` is no
  longer read anywhere; a stray file is surfaced as a shadowed source and
  never loaded.
- [x] Make LSP edits document-version-aware and prove that an unsaved
  configuration buffer cannot be overwritten.

## Protocol and workspace operations {#CONFIGEDITOR-PLAN-PROTOCOL}

- [x] Advertise protocol v1 and register snapshot, preview, apply, occurrences,
  and changed-notification contracts over stdio and WebSocket transports.
- [x] Expand all/code/tag/current-violation/safe-fixability selectors on the
  server and return exact normalized changes plus hypothetical diagnostic
  impact.
- [x] Preload eligible closed files for configuration inventory, including in
  open-files-only analysis, while retaining open-buffer authority.
- [x] Consume previews once, enforce root/revision identity, maintain a
  root-scoped applied overlay, and share reload/recheck/publish/notify logic.
- [x] Page occurrences in stable URI/range/code order with validated cursor and
  limit bounds.
- [x] Accept a validated `rootUri` on the safe `basilisk.fixWorkspace` command,
  restrict its edits to that root, and have the VSIX refresh exact counts after
  it completes.
- [ ] Add Rust LSP E2E coverage for snapshot, preview/apply parity, multi-root
  targeting, stale revision, malformed/read-only source, client rejection,
  external watcher refresh, and exactly one recheck/notification.

## Strict-first adoption {#CONFIGEDITOR-PLAN-ADOPTION}

- [x] Advertise Strict, Maximum, and Suppression audit as one-shot LSP recipes
  that expand to ordinary explicit severities.
- [x] Guide the VS Code sequence through target preset, root-scoped safe fixes,
  refreshed `WithoutSafeFix` review, and explicit severity/path exceptions.
- [x] Persist LSP and CLI adoption as exact-file warning entries with
  `adoption = true` in the active config, leaving new files unaffected.
- [x] Wire post-save graduation through the shared transaction: remove adopted
  rules whose final diagnostic disappeared and clean empty exact-file entries.
- [ ] Add focused and E2E graduation coverage for last-occurrence retention,
  cleanup, rejected edits, restart durability, and multi-root isolation.
- [ ] Add E2E coverage proving safe fixes complete and the snapshot refreshes
  before `WithoutSafeFix` expands; the two operations remain intentionally
  sequential rather than one atomic request.

## Suppression diagnostics {#CONFIGEDITOR-PLAN-SUPPRESSIONS}

- [x] Ship `BSK-0060`–`BSK-0063` as an off-by-default, tag-selectable family
  with independently configurable severity.
- [x] Parse directives once, apply only valid directives, append audit
  diagnostics after suppression, and prevent self-suppression.
- [x] Cover all severity values, blanket/specific/unused/malformed
  classification, line/block/file spans, malformed syntax that must not apply,
  and PEP 484 type comments.
- [ ] Expose stable directive IDs, raw spelling, validity/usage records, and
  diagnostic-origin/suppressibility metadata on the LSP wire.
- [ ] Add cache-parity and occurrence-navigation E2E coverage for all four
  suppression audit rules.

## VS Code and release gates {#CONFIGEDITOR-PLAN-VSIX}

- [x] Gate one full-width singleton editor on the versioned LSP capability and
  keep snapshot/preview/occurrence state in the shared Signals store.
- [x] Render typed tag facets, virtualized rule rows, all severity/reset
  controls, exact previews, presets, path inventory, strict-first actions, and
  paged occurrence navigation as a thin LSP shell.
- [x] Render the Project view's Caching panel: the persistent cache's toggle
  and folder-picker (with a reset that REMOVES the key), plus the read-only
  in-session Salsa rows, driven by a real-webview DOM journey
  (`configuration-editor-cache-dom.test.ts`).
- [x] Enforce the default-deny CSP, nonce-only scripts, no remote resources,
  runtime intent decoding, root-checked navigation, and stale async-result
  rejection in focused VSIX tests.
- [x] Add a headed screenshot scenario that opens the real editor against the
  real LSP and waits for a configuration snapshot.
- [x] Capture and add the real-LSP `vscode-configuration-editor.png` after
  visually verifying its tag facets and per-rule severity controls.
- [x] Embed the configuration-editor capture in shipped documentation and
  extend the website image decode/render guard.
- [ ] Record keyboard, screen-reader, theme, 200% zoom, reduced-motion, CSP, and
  injection verification; close any failures.
- [ ] Add Neovim and Zed clients over the same LSP operations.
- [ ] Pass Rust, VSIX, website, formatting, lint, coverage, mutation, benchmark,
  and full repository CI gates.
