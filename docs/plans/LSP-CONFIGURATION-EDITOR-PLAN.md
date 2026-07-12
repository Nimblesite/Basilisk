# Configuration Editor — Implementation Plan {#CONFIGEDITOR-PLAN}

> Spec: [LSP-CONFIGURATION-EDITOR-SPEC.md](../specs/LSP-CONFIGURATION-EDITOR-SPEC.md)
> (`CONFIGEDITOR`). Shared protocol: [LSPARCH-CONFIG-EDITOR](../specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-EDITOR).

## Outcome {#CONFIGEDITOR-PLAN-OUTCOME}

Ship a tag-first VS Code configuration editor backed by editor-neutral LSP
operations. A user can enable every rule, optionally promote everything to
error, run safe fixes, inspect remaining debt, and deliberately demote or
disable only what cannot be fixed now. Every rule—including suppression audit
rules—has configurable Error/Warning/Info/Disabled severity.

The VSIX contains presentation and message routing only. The reusable Rust
domain owns catalog data, selectors, precedence, preview, config mutation,
adoption, occurrence queries, and refresh.

## Audit baseline {#CONFIGEDITOR-PLAN-BASELINE}

The July 2026 audit found useful foundations but no editor or plan:

- `RuleSeverity` and global/path severity application are implemented and
  tested.
- Runtime tags are canonical, but there is no complete public rule catalog.
- opt-in tag gating runs before severity, so a per-rule `error`/`warning`/`info`
  override does not currently enable a Basilisk rule.
- `basilisk.disableRule` accepts arbitrary severity but performs an unvalidated,
  first-root, direct string edit of `pyproject.toml`, even when
  `basilisk.json` shadows it.
- configuration is split across `BasiliskConfig` and `WorkspaceConfig`, parse
  errors and unknown values silently fall back, and JSON lacks path overrides.
- adoption currently uses a separate sidecar and does not yet run safe fixes,
  persist through normal republish, or auto-graduate in production. The target
  deletes the sidecar and stores exact-file severities in the active config.
- suppression parsing discards spans/validity/usage, so active and unused ignores
  cannot be surfaced as diagnostics.
- the VSIX has reusable Signals and hardened webview primitives, but no
  configuration surface.

These are prerequisites, not details the UI may work around.

## Contract and generated model {#CONFIGEDITOR-PLAN-CONTRACT}

- [x] Add the cohesive product spec and this plan.
- [x] Add `models/configuration_editor.td` and render its SVG.
- [ ] Generate Rust and TypeScript DTOs with typeDiagram; commit the generated
  files beside their owning modules and add a drift test that regenerates/diffs.
- [ ] Add protocol capability version `1` under
  `ServerCapabilities.experimental.basilisk.configurationEditor`.
- [ ] Add every implementation/test reference to the relevant `CONFIGEDITOR-*`
  and `LSPARCH-CONFIG-EDITOR-*` IDs.

## Canonical rule catalog {#CONFIGEDITOR-PLAN-CATALOG}

- [ ] Introduce one public `RuleDescriptor` registry in `basilisk-checker` with
  code, title, summary, docs URL, default severity, default-enabled state, tags,
  and fix capability.
- [ ] Build descriptors from the live `all_rules()` registry. Unknown codes must
  be validation errors, not silently classified as `pep`.
- [ ] Keep tags attached to rules and expose tag kind/vocabulary without a
  second code list ([CHKTAG]). Add the `suppressions` descriptive tag.
- [ ] Replace LSP safe/all-fix code lists with descriptor fix metadata, or one
  shared registry projection; no parallel fixability source.
- [ ] Make `scripts/gen_rules_reference.py` / website data consume the same
  descriptor source instead of becoming a runtime authority.
- [ ] Coarse parity test: every live emitted code has exactly one descriptor,
  canonical tags, valid docs URL, and default severity.

## One validated configuration domain {#CONFIGEDITOR-PLAN-DOMAIN}

- [ ] Consolidate the analysis projection now split between `BasiliskConfig`
  and LSP `WorkspaceConfig`; remove the dead `strict` mode field and preserve
  editor-only settings separately.
- [ ] Return active source, shadowed sources, format, field-level provenance,
  and a content-hash revision from config discovery.
- [ ] Define one lowercase serde representation for the four severities; reject
  invalid severity, unknown rules/tags/fields, and malformed config with
  structured `ConfigurationProblem`s.
- [ ] Make an explicit non-disabled per-rule override enable an opt-in rule;
  `disabled` deselects it; removing the entry inherits tag/default selection.
- [ ] Resolve overlapping path patterns deterministically and collapse
  `disabled[]` versus `rules.CODE = "disabled"` to one effective model.
- [ ] Implement structure-aware, comment/order/newline-preserving TOML/JSON
  patches. Target the active source; offer explicit migration when a
  `basilisk.json` shadows `pyproject.toml`.
- [ ] Represent editor-generated per-file adoption as exact-file
  `per-path-overrides` entries with `adoption = true` and ordinary `rules`
  severities in the same active config file. No second persistence file.
- [ ] Validate the complete patched document before emitting it. The reusable
  service returns a patch; CLI writes use temp+atomic rename, LSP writes use one
  versioned `WorkspaceEdit` so unsaved buffers and undo remain correct.
- [ ] Config-domain tests: all severities, enablement, reset, source precedence,
  malformed/unknown input, comments, inline/dotted TOML, JSON, newline style,
  overlapping paths, stale revision, and no-op idempotence.
- [ ] Keep the implementation compatible with the planned `lspkit-config`
  migration; do not duplicate generic ancestor-loading infrastructure.

## Typed LSP read/preview/apply API {#CONFIGEDITOR-PLAN-PROTOCOL}

- [ ] Implement `basilisk/configurationSnapshot` with explicit `rootUri`, rule
  catalog, tags, configured/effective state, provenance, counts, debt, and
  config problems.
- [ ] Implement selectors: all, exact codes, any/all tags, current violations,
  safe-fixable, and without-safe-fix.
- [ ] Implement `basilisk/previewConfigurationChange`: expand selectors to a
  stable code list, apply mutations to an in-memory config, run cancellable
  hypothetical Salsa analysis, optionally calculate safe fixes, and return
  exact impact without writing.
- [ ] Implement `basilisk/applyConfigurationChange`: require preview ID plus
  matching base revision, apply one workspace edit, reload the selected root,
  update Salsa input, recheck, republish, and return the fresh snapshot.
- [ ] Implement `basilisk/ruleOccurrences` for tag/rule/debt navigation with
  paging or streaming so large workspaces do not flood one response.
- [ ] Send `basilisk/configurationChanged` after API applies and external config
  changes. Watch the active `pyproject.toml` or `basilisk.json` in every analysis
  scope, including `openFilesOnly`.
- [ ] Route the legacy `basilisk.disableRule` command through the same validated
  service, then deprecate its misleading name after all code actions migrate.
- [ ] E2E: multi-root targeting, all-on/all-off/tag bulk, native versus maximum,
  reset, preview/apply selector parity, stale conflict, invalid/read-only source,
  client edit rejection, watcher refresh, and exactly one republish/notification.

## Make adoption production-correct {#CONFIGEDITOR-PLAN-ADOPTION}

- [ ] Apply adoption overrides on every open/change/save/scan/recheck path, not
  only the adopt command's one-off publish.
- [ ] Remove `AdoptionStore` and `.basilisk/adoptions.toml`. Read/write adoption
  through the same root-aware, revision-checked configuration service as every
  other rule severity; never store another root's file under the selected root.
- [ ] Implement the strict-first transaction: hypothetical all-rules config →
  SafeFix workspace edit → reanalysis → remaining error ledger → explicit
  per-file demotion preview/apply.
- [ ] Call auto-graduation in production after reanalysis; remove a rule from
  the exact-file config entry only when no matching violation remains, remove
  the empty adopted entry, then notify/refresh the editor.
- [ ] Preserve the separate choices: per-file adoption keeps new files strict;
  project/path demotion affects future code and must say so in preview.
- [ ] E2E: safe fix occurs first, remaining errors demote, normal edits retain
  demotion, new files stay strict, fixed debt graduates, restart reloads debt,
  and multi-root stores remain isolated.

## Suppression diagnostics {#CONFIGEDITOR-PLAN-SUPPRESSIONS}

- [ ] Replace the lossy suppression model with rich directives carrying stable
  ID, exact span/raw spelling, scope, severity action, explicit/blanket selector, validity,
  and paired block delimiters.
- [ ] Apply directives in a batch and return a usage ledger: matched codes,
  diagnostic count, and whether severity/output actually changed.
- [ ] Validate codes through the canonical rule catalog; diagnose malformed
  verbs/brackets, unknown codes, conflicts, misplaced file directives,
  unmatched ends, and unclosed blocks.
- [ ] Add `BSK-I0060` active-specific, `BSK-W0061` blanket,
  `BSK-W0062` unused, and `BSK-E0063` malformed. Tag every rule
  `basilisk` + `suppressions`, keep the family off in the unconfigured default,
  and generate normal rule documentation.
- [ ] Append audit diagnostics after inline suppression so a directive cannot
  suppress its own audit. Project/path severity configuration still applies.
- [ ] Carry diagnostic origin/suppressibility through incremental cache types.
- [ ] Index these diagnostics for Problems and `ruleOccurrences`.
- [ ] Tests: line/block/file directives, active/unused/blanket/malformed,
  foreign checker code fallback, duplicate/conflicting directives, no
  self-suppression, all four configured severities, cache parity, and workspace
  occurrence navigation.

## VSIX shell {#CONFIGEDITOR-PLAN-VSIX}

- [ ] Add client-only `basilisk.openConfigurationEditor` and a settings-gear
  action in the Basilisk activity view; gate on server capability/version.
- [ ] Add `configuration-editor.ts` (singleton host/intent routing),
  `configuration-editor-document.ts` (CSP shell), generated model types, and
  focused render modules. Keep every file under 500 LOC and deduplicate against
  `profiler-webview.ts` before adding another host abstraction.
- [ ] Extend the single VSIX Signals store with snapshot/loading/error/revision
  state and explicit actions. No mutable panel-local cache.
- [ ] Ready handshake → host `postMessage` snapshot. Runtime-decode all webview
  intents; forward them to the LSP without parsing config or evaluating rules.
- [ ] Build Overview, tag rail, virtualised Rules table, rule detail/occurrences,
  Adoption, Path Overrides, and Project source/provenance views.
- [ ] Add labelled severity controls, inherited reset, bulk tray, native/maximum/
  disable-all actions, impact review, conflict refresh, raw-config navigation,
  and clear global-versus-per-file consequences.
- [ ] Use VS Code theme tokens/native fonts with restrained Basilisk orange/sky
  accents. Support high contrast, 200% zoom, responsive narrow layout, reduced
  motion, full keyboard navigation, focus restoration, and `aria-live` status.
- [ ] Default-deny CSP, nonce-gated local scripts, no remote resources,
  `localResourceRoots: []`, no hidden retained DOM, and injection tests for
  every workspace-derived string.
- [ ] VSIX tests assert actual open/render/message/apply behavior—never
  `getCommands(true)` or `whenCommandReady` existence checks—and prove the
  extension performs no configuration filesystem writes.

## Visual verification and cross-editor reuse {#CONFIGEDITOR-PLAN-CROSS-EDITOR}

- [ ] Capture `vscode-configuration-editor.png` from the real headed VSIX after
  the feature ships; embed it on the configuration documentation page and add
  the normal decode/render E2E guard.
- [ ] Run keyboard and screen-reader audits on macOS/Windows, all built-in theme
  classes, 200% zoom, and reduced motion. Fix every blocker before release.
- [ ] Add Neovim and Zed commands/views over the same snapshot/preview/apply API;
  no second rule/config implementation.
- [ ] Document the shipped UI in the VSIX README and website only after its
  capability is present in the bundled LSP.

## Verification gate {#CONFIGEDITOR-PLAN-VERIFICATION}

- [ ] Run `deslop:find-similar` before implementation and
  `deslop:top-offenders` after; merge shared config/webview/catalog logic.
- [ ] `cargo fmt --check`, strict clippy, Rust unit/e2e, VSIX compile/lint/tests,
  website build/e2e, and the repository's full CI target pass.
- [ ] Coverage and mutation ratchets only rise; no exclusions or weakened tests.
- [ ] Fresh-project PEP conformance remains unchanged. Maximal policy is an
  explicit user transaction, never the conformance/default configuration.
