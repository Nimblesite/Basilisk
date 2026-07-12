# basilisk-config

Configuration parsing for Basilisk — reads `[tool.basilisk]` from
`pyproject.toml` and the compatibility `basilisk.json` format.

## Role in Basilisk

This crate owns the checker-facing `BasiliskConfig`, severity/path/module
override parsing, shared path matching, and the gradual-adoption store. Current
root-level discovery gives `basilisk.json` priority over `[tool.basilisk]`; the
sources are not merged. LSP/editor settings such as analysis mode live in
`basilisk-lsp` today and are not parsed by this crate.

## Key concepts

- **Per-path overrides** — disable specific rules or override their severity for matching path globs (e.g. legacy directories).
- **Global severity overrides** — set an enabled rule to `error`, `warning`,
  `info`, or `disabled`.
- **Import-resolution overrides** — `stub-paths` prepends user stub directories (resolution step 1); `typeshed-path` replaces the vendored standard-library typeshed wholesale as the canonical step-3 source ([STUBRES-CUSTOM-TYPESHED](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED)).
- **Adoption persistence** — records per-file demotions in
  `.basilisk/adoptions.toml` for the LSP/CLI adoption flow.

The crate does **not** currently migrate mypy/Pyright configuration or own LSP
analysis modes. Those are separate planned/consumer concerns.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `serde` | Deserialization |
| `serde_json` | JSON parsing |
| `toml` | TOML parsing |

## Status

Parsing is consumed by `basilisk-checker`, `basilisk-cli`, and `basilisk-lsp`.
Validated lossless mutation, source provenance, revision safety, and the editor
API are planned in
[`LSP-CONFIGURATION-EDITOR-PLAN.md`](../../docs/plans/LSP-CONFIGURATION-EDITOR-PLAN.md).
