# basilisk-config

Configuration parsing for Basilisk — reads `[tool.basilisk]` from
`pyproject.toml`, the only configuration source.

## Role in Basilisk

This crate owns the checker-facing `BasiliskConfig`, severity/path/module
override parsing and shared path matching. Discovery walks **up** from each
checked file: every ancestor `pyproject.toml` carrying a `[tool.basilisk]`
table contributes, and the tables merge cumulatively with the nearest file
winning per key — a nested table refines its ancestors, never replaces them
([CHKARCH-CONFIG-DISCOVERY]). A stray legacy `basilisk.json` is never read;
the config editor reports it in `shadowed_sources`. LSP/editor settings such
as analysis mode live in `basilisk-lsp` today and are not parsed by this
crate.

## Key concepts

- **Per-path overrides** — disable specific rules or override their severity for matching path globs (e.g. legacy directories).
- **Global severity overrides** — set an enabled rule to `error`, `warning`,
  `info`, or `disabled`.
- **Import-resolution overrides** — `stub-paths` prepends user stub directories (resolution step 1); `typeshed-path` replaces the vendored standard-library typeshed wholesale as the canonical step-3 source ([STUBRES-CUSTOM-TYPESHED](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED)).
- **Adoption target** — exact-file `per-path-overrides` entries in the active
  config carry generated demotions. All rule configuration stays in that one
  file; there is no separate adoption sidecar.

The crate does **not** currently migrate mypy/Pyright configuration or own LSP
analysis modes. Those are separate planned/consumer concerns.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `serde` | Deserialization |
| `toml` | TOML parsing |
| `toml_edit` | Format-preserving TOML edits for the editor API |

## Status

Parsing is consumed by `basilisk-checker`, `basilisk-cli`, and `basilisk-lsp`.
Validated mutation, ancestor-walk cumulative discovery, content revisions, and
the editor API are implemented; the editor targets `pyproject.toml` only and
surfaces a stray `basilisk.json` as an ignored shadowed source. Remaining
provenance, document-version safety, and domain consolidation work is tracked
in
[`LSP-CONFIGURATION-EDITOR-PLAN.md`](../../docs/plans/LSP-CONFIGURATION-EDITOR-PLAN.md).
