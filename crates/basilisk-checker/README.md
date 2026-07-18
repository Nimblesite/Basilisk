# basilisk-checker

Core type checking rules and diagnostic emission for Basilisk.

## Role in Basilisk

This is the **third stage** of the analysis pipeline. After
`basilisk-resolver` builds the scope tree and resolves names, the checker walks
the AST with resolved type information and emits configured diagnostics.

```
AST + resolved scopes + effective config ➜ [basilisk-checker] ➜ diagnostics
```

## Key concepts

- **Configuration, not modes** — the unconfigured default enables the complete
  core PEP rule set; Basilisk-specific house rules are opt-in by their live rule
  tags. There is no basic/standard/strict mode.
- **Per-rule severity** — enabled rules can report as error, warning, info, or
  disabled globally and per path. Inline and per-file directives sit above
  project configuration in the precedence ladder.
- **Live tagged registry** — provenance, PEP-category, and descriptive tags are
  attached to rules and drive selection/classification.
- **Stub resolution** — resolves types via `basilisk-stubs`: the standard
  library from the pinned step-3 typeshed source—custom path, explicit commit,
  verified `main`, or bundled names-only fallback—followed by third-party
  packages in the specified order
  ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
- **Gradual adoption** — project/path configuration and the LSP adoption store
  let existing codebases record debt without changing the default rule set.

## Diagnostic rules

The registry is intentionally not copied into this README. See the generated
[diagnostic reference](https://www.basilisk-python.dev/docs/rules/) and
[`CHECKER-RULE-TAGGING-SPEC.md`](../../docs/specs/CHECKER-RULE-TAGGING-SPEC.md)
for the canonical tag model.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `basilisk-parser` | AST input |
| `basilisk-resolver` | Scope and name resolution |
| `basilisk-config` | Per-path configuration |
| `basilisk-stubs` | Type stub resolution |

## Status

The checker and severity engine are shipped. The canonical rule-catalog API,
strict-first adoption transaction, opt-in suppression diagnostics, and visual
configuration editor are tracked in
[`LSP-CONFIGURATION-EDITOR-PLAN.md`](../../docs/plans/LSP-CONFIGURATION-EDITOR-PLAN.md).
