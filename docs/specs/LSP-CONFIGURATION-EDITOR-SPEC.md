# Configuration Editor {#CONFIGEDITOR}

**Status:** target specification; the foundations exist, but the editor and its
transactional LSP API are not shipped yet. The implementation sequence is in
[LSP-CONFIGURATION-EDITOR-PLAN.md](../plans/LSP-CONFIGURATION-EDITOR-PLAN.md).

Basilisk needs a configuration experience that makes the strongest useful
policy easy to adopt without pretending an established codebase can fix every
diagnostic today. The first client is the VSIX, but configuration knowledge,
rule selection, impact analysis, persistence, and adoption all live behind a
reusable LSP API. The VSIX is a small, beautiful rendering shell over that API.

This document owns the end-to-end experience. The sources of truth it composes
remain:

- severity, scope, and precedence: [CHKARCH-STRICTNESS](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-STRICTNESS);
- the flat tag model: [CHKTAG](CHECKER-RULE-TAGGING-SPEC.md#CHKTAG);
- shared LSP wire methods: [LSPARCH-CONFIG-EDITOR](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-EDITOR);
- safe fixes and per-file adoption: [AUTOFIX](LSP-MASS-AUTOFIX-SPEC.md#AUTOFIX);
- VS Code-only hosting: [VSIX-CONFIGURATION-EDITOR](VSIX-SPEC.md#VSIX-CONFIGURATION-EDITOR).

## Product principles {#CONFIGEDITOR-PRINCIPLES}

1. **Tags are the information architecture.** Rules are browsed, filtered,
   summarised, and bulk-edited through their authoritative tags. The UI never
   invents a parallel category list or guesses provenance from a code prefix.
2. **No modes.** “Maximum policy”, “enable all”, and “disable all” are named
   transactions that expand to explicit rule operations. They do not create a
   hidden `strict`/`standard` mode ([CHKARCH-CONFIGURATION-ONLY]).
3. **Severity and enablement are one understandable control.** Every rule can be
   `error`, `warning`, `info`, or `disabled`. “Inherited” removes an explicit
   override; it is not a fifth severity.
4. **Strict target, explicit debt.** Users can enable the complete rule catalog,
   run safe fixes, and then deliberately demote or disable only the debt they
   cannot address now. Existing exceptions stay visible and measurable.
5. **Preview before mutation.** The server expands selectors, validates the
   active config, runs hypothetical analysis, and returns the exact affected
   rules/files/counts before any write.
6. **The LSP owns policy.** Clients send intent and render results. They never
   enumerate rules, parse configuration, calculate precedence, or write files.
7. **One project config.** Rule severities, tag opt-ins, path/file exceptions,
   and generated adoption debt are persisted only in the root's active config
   file—not VS Code settings, extension state, a sidecar, or a named mode.

## Current foundation and missing pieces {#CONFIGEDITOR-STATUS}

| Capability | Today | Required target |
|---|---|---|
| Four severities | Shipped in `basilisk-config::RuleSeverity` | Keep one canonical wire/disk representation |
| Global/path overrides | Parsed and applied | Lossless validated read/write/reset with provenance |
| Rule tags | Live checker source of truth | Expose full tagged rule catalog through the LSP |
| Single-rule project edit | `basilisk.disableRule` performs an unsafe first-root string edit | Revision-checked preview/apply transaction against the active source |
| Bulk rules | None | All/code/tag/diagnostic/fixability selectors |
| Adoption persistence | A separate sidecar exists, but normal republish and production graduation are incomplete | Exact-file severities in the active config file; no sidecar or adoption mode |
| Ignore visibility | Directives only hide or demote diagnostics | First-class, workspace-indexed suppression diagnostics |
| VSIX editor | None | Full-width accessible editor-tab webview |

An explicit non-disabled per-rule severity MUST enable an opt-in `basilisk`
rule even when its broader tag gate is off. `disabled` explicitly deselects it;
removing the override returns it to inherited tag/default selection. Without
this rule, a per-rule severity editor would display a value that has no effect.

## Tag-first rule model {#CONFIGEDITOR-TAGS}

The landing view is a tag dashboard. It exposes the three tag kinds without
flattening them into a fake hierarchy:

- **provenance:** `pep`, `basilisk`;
- **PEP category:** the reserved `python/typing` category tags;
- **descriptive:** `strictness`, `style`, `redundancy`, `dependencies`,
  `imports`, `stubs`, `suppressions`, and future checker-declared tags.

A rule may appear in more than one descriptive view. Tag totals therefore are
facets, not numbers to add together. Every tag tile shows its rule count,
current diagnostic count, and severity distribution. Selecting a tile filters
the rule table; applying a bulk action sends the tag selector to the server.
The preview response includes `expandedRuleCodes`, making the exact transaction
reviewable and stable.

The canonical catalog comes from the live rule registry and supplies code,
title, summary, documentation URL, default severity, default-enabled state,
tags, and fix metadata. The VSIX and website MUST NOT maintain another list.

## Severity semantics {#CONFIGEDITOR-SEVERITY}

Each rule row presents:

| UI state | Persisted meaning |
|---|---|
| Inherited | No override; follow the next-lower precedence source |
| Error | Enable and report as an error |
| Warning | Enable and report as a warning |
| Info | Enable and report as information |
| Disabled | Do not report the rule |

“Native severity” is a bulk-operation intent: the LSP expands it to each rule's
own default severity. “Maximum policy” instead promotes every selected rule to
`error`. Neither value is stored as a fifth severity.

The rule detail view explains both `configuredSeverity` and
`effectiveSeverity`, including the winning global/path/file/adoption source.
An inherited control must never look identical to an explicit override.

## Strict-first adoption {#CONFIGEDITOR-ADOPTION}

The primary adoption workflow is one continuous workspace view, not a wizard:

1. **Set the target.** Preview “Enable every rule at native severity” or
   “Maximum policy”. The server analyses the proposed configuration, including
   rules that are currently disabled.
2. **Take the free wins.** Preview and run `SafeFix` changes as one undoable
   workspace edit ([AUTOFIX-CLASSIFY]).
3. **Review remaining debt.** Group remaining occurrences by tag, rule, file,
   severity, and fixability. “Without safe fix” is a filter, never an automatic
   decision to disable.
4. **Choose the exception.** Set selected rules to warning/info/disabled at the
   project or path scope, or adopt only the current per-file debt. Per-file
   adoption writes ordinary exact-file severity entries into the active config.
   The preview states plainly that a global disable also hides future violations.
5. **Apply once.** The LSP writes one revision-checked configuration edit,
   reloads analysis, republishes diagnostics, and returns the fresh snapshot.
6. **Pay debt down.** The Adoption view shows every exception. Per-file entries
   auto-graduate when the last matching violation is fixed.

No operation silently disables every rule that happens to fire. The user sees
and confirms the exact rule set. New files remain fully checked when the user
chooses per-file adoption instead of a global downgrade.

## Suppressions are diagnostics {#CONFIGEDITOR-SUPPRESSIONS}

Every source suppression or severity directive is auditable at its comment
location through the rule family in
[CHKARCH-STRICTNESS-SUPPRESSION-DIAGNOSTICS](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-STRICTNESS-SUPPRESSION-DIAGNOSTICS).
This makes ignores searchable in Problems and in the configuration editor's
workspace occurrence list.

The family uses the descriptive `suppressions` tag and separates four policies.
All four are `basilisk` opt-in rules: the unconfigured default emits **no
suppression-audit diagnostics**. The severities below are their native values
only after the family or an individual rule is enabled:

| Rule | Native severity when enabled | Meaning |
|---|---|---|
| `BSK-I0060` | Info | Valid code-specific directive that actively suppresses or changes severity |
| `BSK-W0061` | Warning | Active blanket directive with no Basilisk rule selector |
| `BSK-W0062` | Warning | Directive that matches no diagnostic or changes nothing |
| `BSK-E0063` | Error | Malformed, unknown, conflicting, or unpaired directive |

Each is configurable to any severity. A team can opt into the family, keep
active specific ignores at `info`, promote blanket/unused ignores to `error`,
or turn the family back off. Audit diagnostics are appended after ordinary inline suppression and
cannot be hidden by the directive they describe; project/path configuration is
the deliberate way to change their severity.

## LSP-owned operations {#CONFIGEDITOR-OPERATIONS}

The shared methods are specified in
[LSPARCH-CONFIG-EDITOR-PROTOCOL](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-EDITOR-PROTOCOL).
Together they provide:

- resolved snapshot and configuration provenance;
- tag-aware rule catalog, effective severity, counts, and fixability;
- selectors for all rules, exact codes, tags, current violations, safe-fixable
  occurrences, and occurrences without a safe fix;
- batch set/reset/enable/disable at project or path scope;
- hypothetical analysis and safe-fix impact preview;
- revision-checked apply, deterministic reload/recheck/republish;
- workspace occurrences for navigation;
- a configuration-changed notification for every editor.

Every request includes `rootUri`; silently choosing the first workspace root is
forbidden. Unknown rules/tags/severities, invalid config, shadowed sources,
read-only files, and stale revisions are structured errors rather than ignored
input.

## Data model {#CONFIGEDITOR-MODEL}

The language-neutral source is
[`models/configuration_editor.td`](../../models/configuration_editor.td), rendered
as [`docs/models/configuration_editor.svg`](../models/configuration_editor.svg).
Rust and TypeScript DTOs are generated from the same typeDiagram model during
implementation; handwritten wire-shape copies are forbidden.

The important distinction is:

- `RuleSeverity`: the four persisted severities;
- `RuleSetting`: a mutation intent, adding `Inherit` (remove override) and
  `Native` (expand to each selected rule's default);
- `RuleState.configuredSeverity`: optional explicit value;
- `RuleState.effectiveSeverity`: result after the complete precedence ladder.

## Configuration sources and writes {#CONFIGEDITOR-SOURCES}

The snapshot names the active source and all shadowed sources. Existing
`basilisk.json` remains readable and has current loader priority; new editor-
created configuration uses `[tool.basilisk]` in `pyproject.toml`. The editor
must mutate the active source or offer an explicit migration—it must never
write an ignored `pyproject.toml` while `basilisk.json` is active.

All project policy—including editor-generated per-file adoption entries—lives
in that one active config file. The target design has no
`.basilisk/adoptions.toml`, no hidden workspace state, and no adoption/strictness
mode. `adoption = true` on an exact-file path entry records provenance while its
`rules` table remains ordinary severity configuration ([AUTOFIX-ADOPTION-FILE]).

The writer is structure-aware and preserves unrelated keys, comments, ordering,
and newline style. It validates the complete result before returning a single
versioned `WorkspaceEdit`; the LSP does not write behind an unsaved editor
buffer. The apply request rejects a stale `baseRevision` and asks the user to
refresh or re-preview. Malformed configuration is read-only until fixed; it is
never replaced with defaults.

## VSIX experience {#CONFIGEDITOR-VSIX-EXPERIENCE}

The VSIX opens one full-width editor tab from **Basilisk: Open Configuration
Editor** and a settings action in the Basilisk activity view. A narrow tree view
is not suitable for the rule catalog, and the extension must not take over all
`pyproject.toml` files as a custom editor.

```text
┌ Configuration · workspace ─ active: pyproject.toml ─ Saved ─ Open raw ┐
│ Overview  Rules  Adoption  Path overrides  Project                    │
├ Tags ───────────────┬ Rules ────────────────────────────────┬ Detail ─┤
│ pep          148    │ Search: tag:suppressions fix:none     │ BSK-…   │
│ basilisk      13    │ [✓] Code · summary · tag chips  [▼]  │ source  │
│ strictness     9    │ [ ] …                                 │ impact  │
│ suppressions   4    │                                       │ files   │
├─────────────────────┴───────────────────────────────────────┴──────────┤
│ 12 selected · Set severity · Reset · Preview changes                 │
└───────────────────────────────────────────────────────────────────────┘
```

The overview shows exact counts for Error, Warning, Info, Disabled, and
Inherited plus workspace occurrences. It does not invent a “strictness score”.
Rows are virtualised, searchable, and keyboard navigable. Search supports text
and facets such as `tag:strictness`, `severity:error`, `status:disabled`,
`has:diagnostics`, and `fix:none`; selector evaluation remains server-owned for
bulk mutations.

Visual polish uses VS Code theme variables and the Basilisk orange/sky accents,
never a fixed light/dark canvas. Controls remain legible in high-contrast themes,
at 200% zoom, and with reduced motion. The shell follows the official
[webview UX guidance](https://code.visualstudio.com/api/ux-guidelines/webviews),
[theme tokens](https://code.visualstudio.com/api/references/theme-color), and
[webview security guidance](https://code.visualstudio.com/api/extension-guides/webview#security).

## Accessibility and security {#CONFIGEDITOR-ACCESSIBILITY-SECURITY}

- Semantic headings, landmarks, tables/lists, labels, and real buttons/selects;
  no click-only `div` or table row.
- Complete keyboard operation, visible focus, focus preservation after refresh,
  and an `aria-live` region for preview/apply/conflict status.
- Severity is always text-labelled; colour is redundant.
- `prefers-reduced-motion` disables non-essential transitions.
- Default-deny CSP, nonce-gated local scripts, no remote resources,
  `localResourceRoots: []`, and no retained hidden state.
- The host sends data only after a ready handshake. Every inbound message is
  runtime-decoded and revalidated by the LSP; workspace text is never injected
  into executable HTML.
- The extension host may open/navigate files and apply the server's
  `WorkspaceEdit`; it never parses or writes TOML/JSON itself.

## Acceptance criteria {#CONFIGEDITOR-ACCEPTANCE}

The feature is complete only when:

1. catalog parity proves every live rule appears once with its canonical tags;
2. each rule supports Error/Warning/Info/Disabled plus reset to Inherited;
3. an explicit severity enables an opt-in rule;
4. all/code/tag/fixability selectors preview and apply the exact same code set;
5. enable-all, maximum, and disable-all work in multi-root workspaces without
   first-root fallbacks;
6. stale, malformed, shadowed, and read-only configs cannot be overwritten;
7. apply triggers reload, Salsa invalidation, recheck, publish, and notification
   in every analysis scope;
8. strict-first adoption runs safe fixes, persists demotions on later edits, and
   auto-graduates;
9. all four suppression-audit rules are workspace-findable and severity-tunable;
10. the VSIX performs no configuration filesystem writes and passes keyboard,
    screen-reader, light/dark/high-contrast, zoom, CSP, and injection tests;
11. the real editor screenshot is captured and verified only after the feature
    ships—never mocked from static HTML.
