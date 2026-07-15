# Mass autofix and adoption {#AUTOFIX}

This document records the shipped batch-fix engine, the root-scoped safe-fix
operation used by the configuration editor, and active-configuration adoption.

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

The configuration editor always supplies its selected root to the safe-only
`basilisk.fixWorkspace` command.

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
combinability flag, or source enum. Callers select the static safe/all rule list
before collecting edits. Configuration-editor snapshots and occurrences
project safe/unsafe counts from those same lists.

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

## Configuration seeding and fixes {#AUTOFIX-STRICT-FIRST}

There is no preset workflow. Seeding already materializes a strict starting
point: a project with no config gets every PEP rule at `error` and every house
rule at `warning`, written explicitly
([CONFIGEDITOR-SEEDING](LSP-CONFIGURATION-EDITOR-SPEC.md#CONFIGEDITOR-SEEDING)).
Tightening or relaxing from there is ordinary bulk mutation through
preview/apply, while root-scoped `basilisk.fixWorkspace` applies the currently
safe fixes as a separate, reviewable source edit. Unsafe fixes are never
included implicitly, and no rule is disabled without an ordinary configuration
preview/apply writing an explicit `disabled` entry.

## Gradual adoption {#AUTOFIX-ADOPTION}

Adoption records current file debt as ordinary exact-path rule overrides in the
active configuration. The `adoption = true` marker identifies editor-generated
debt for snapshots and activity views; its severities participate in normal
configuration resolution.

### Current flow {#AUTOFIX-ADOPTION-FLOW}

`basilisk.adoptFile` and `basilisk.adoptWorkspace` read current error and
safety-violation codes from the workspace index, write warning-severity
exact-file overrides through the configuration-editor transaction, then reload
and recheck. The workspace command groups files by their owning root and writes
each root's active config independently. `basilisk.unadoptFile` removes the
generated rules for one file through the same refresh path.

On `textDocument/didSave`, the LSP rechecks the saved file and compares its
current codes with its adopted rule set. A rule graduates when its last matching
diagnostic is gone: that explicit warning entry is removed, an empty generated
file entry is cleaned up, diagnostics are republished, and
`basilisk/configurationChanged` refreshes clients.

The direct adopt commands record the diagnostics they receive; they do not run
safe fixes first. Users who want safe-fix-first adoption use the explicit
configuration-editor sequence above, so the post-fix inventory is visible and
reviewable.

The CLI `basilisk adopt`, `basilisk unadopt`, and `basilisk adopt --status`
operate on the same active configuration representation. CLI adoption is
durable but does not perform LSP post-save graduation while no server is
running.

### Configuration format {#AUTOFIX-ADOPTION-FILE}

For a `pyproject.toml` active source, a generated entry has this shape:

```toml
[tool.basilisk.per-path-overrides."src/utils.py"]
adoption = true

[tool.basilisk.per-path-overrides."src/utils.py".rules]
BSK-E0001 = "warning"
BSK-E0003 = "warning"
```

Paths are relative to the config file's directory (the discovered config
root), and the structure-aware writer retains unrelated configuration content.
No separate adoption file is read or written.

### Behavior and boundaries {#AUTOFIX-ADOPTION-RULES}

- Only explicitly recorded file/code pairs are demoted.
- New files are unaffected by existing exact-file exceptions.
- Manual un-adoption, durable rechecks, post-save graduation, and empty-entry
  cleanup are implemented.
- Graduation is driven by an LSP save/recheck. It is not a background CLI
  migration process.
- Safe fixing, adoption, and later config demotion are distinct reviewable
  operations, not one atomic command.

### VS Code surface {#AUTOFIX-ADOPTION-VSCODE}

The server advertises Adopt File, Adopt Workspace, and Un-adopt File. The
activity panel derives adopted-file state from the active configuration.
Adoption is not part of the configuration-editor contract
([CONFIGEDITOR-ACCEPTANCE](LSP-CONFIGURATION-EDITOR-SPEC.md#CONFIGEDITOR-ACCEPTANCE)).
