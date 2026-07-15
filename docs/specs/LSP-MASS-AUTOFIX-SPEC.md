# Mass autofix and adoption {#AUTOFIX}

This document records the shipped batch-fix engine, the root-scoped safe-fix
commands, and gradual adoption.

## Mass autofix {#AUTOFIX-MASS}

Single-diagnostic quick fixes are produced by `code_actions/fixes.rs`. The
mass-fix engine combines applicable edits for one file; file and workspace
commands use that engine across their selected scope. The CLI exposes
`basilisk fix [--unsafe] [--rules ...] [path]`.

### Scopes {#AUTOFIX-MASS-OVERVIEW}

- A diagnostic lightbulb applies one fix.
- File actions apply all selected fixable rules in the active file.
- Workspace/CLI actions apply the same selection to each discovered file.
- `basilisk.fixWorkspace` and `basilisk.fixWorkspaceAll` accept an optional
  `{ "rootUri": "file:///..." }` execute-command argument. When supplied, the
  server validates that it is an active workspace root and edits only indexed
  files beneath it. Omitting the argument retains the all-indexed-roots
  behavior used by existing clients.

Any client UI may invoke the commands with a `rootUri`; the configuration
editor exposes no fix affordance of its own.

### Classification {#AUTOFIX-CLASSIFY}

Safety is currently a rule-level allowlist in `code_actions/mass_fix.rs`, not
per-diagnostic metadata.

- Safe default: `BSK-E0001`, `BSK-E0002`, `BSK-E0005`, and `BSK-W0050`.
- All/unsafe additionally includes `BSK-E0003`.

`BSK-E0003` is excluded from the safe set because repeated application can
conflict with redundant-annotation removal. The explicit all/unsafe path
applies its edits immediately; no unsafe-review list is generated.

### Fix representation {#AUTOFIX-METADATA}

Fix functions return ordinary LSP `CodeAction` values containing
`WorkspaceEdit`s. There is no shipped per-diagnostic fix-safety object,
combinability flag, or source enum. Callers select the static safe/all rule
list before collecting edits. Safe/unsafe classification is consumed only by
these mass-fix commands; it appears nowhere in the configuration-editor
protocol ([CONFIGEDITOR-MODEL](LSP-CONFIGURATION-EDITOR-SPEC.md#CONFIGEDITOR-MODEL)).

### VS Code and CLI surface {#AUTOFIX-MASS-VSCODE}

The server advertises `basilisk.fixFile`, `basilisk.fixFileAll`,
`basilisk.fixWorkspace`, and `basilisk.fixWorkspaceAll`. The plain names select
safe rules; the `All` variants widen the rule set.
`source.fixAll.basilisk` is the file code-action kind. The CLI uses safe rules
by default and widens with `--unsafe` or explicit `--rules`.

This is a standalone source-edit operation, never part of a configuration
preview transaction.

### Conflicts {#AUTOFIX-CONFLICTS}

Candidate edits are sorted by start position. The engine greedily retains
non-overlapping edits and silently skips a later overlap. A normal recheck
exposes anything still applicable; there is no internal multi-pass loop.

### Undo {#AUTOFIX-UNDO}

One file-level mass action returns one `WorkspaceEdit`, so the editor can treat
it as one undo operation. Workspace behavior follows the client's handling of
the returned edit.

## Configuration seeding and fixes {#AUTOFIX-SEEDING}

There is no preset workflow. Seeding already materializes a strict starting
point: a project with no config gets every PEP rule at `error` and every house
rule at `warning`, written explicitly
([LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING)).
Tightening or relaxing from there is ordinary bulk mutation through
preview/apply, while root-scoped `basilisk.fixWorkspace` applies the currently
safe fixes as a separate, reviewable source edit. Unsafe fixes are never
included implicitly, and no rule is disabled without an ordinary configuration
preview/apply writing an explicit `disabled` entry.

## Gradual adoption {#AUTOFIX-ADOPTION}

Adoption records current error debt as ordinary warning-severity rule entries
in the config file of the folder that holds the debt — plain code → severity
entries in the one configuration model, with no exact-file overrides,
ownership markers, or sidecar state
([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)).

### Flow {#AUTOFIX-ADOPTION-FLOW}

`basilisk.adoptFile` and `basilisk.adoptWorkspace` read the current error and
safety-violation codes from the workspace index and demote them to `warning`
in the nearest config file governing each affected folder, through the shared
root-aware configuration mutation service — the same reload/recheck/notify
tail as every configuration write. `basilisk.unadoptFile` deletes those folder
entries again, restoring the ancestor severity. Re-running adoption recomputes
the debt and rewrites the entries, so rules that no longer fire in a folder
revert without manual bookkeeping. The CLI `basilisk adopt`,
`basilisk unadopt`, and `basilisk adopt --status` operate on the same
representation.

The adopt commands record the diagnostics they receive; they do not run safe
fixes first. Safe fixing, adoption, and later tightening are distinct
reviewable operations, not one atomic command.

### Behavior and boundaries {#AUTOFIX-ADOPTION-RULES}

- Only rules that currently fire are demoted, and only in the folders where
  they fire.
- A folder entry is a plain override: new files in that folder inherit it,
  exactly like any folder config entry.
- There is no post-save graduation daemon and no background migration process;
  re-running adoption is the explicit, reviewable way to tighten.

### VS Code surface {#AUTOFIX-ADOPTION-VSCODE}

The server advertises Adopt File, Adopt Workspace, and Un-adopt File. The
activity panel derives adoption state from the config files themselves.
Adoption is not part of the configuration-editor contract
([CONFIGEDITOR-ACCEPTANCE](LSP-CONFIGURATION-EDITOR-SPEC.md#CONFIGEDITOR-ACCEPTANCE)).
