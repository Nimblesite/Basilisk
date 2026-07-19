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
- **Import-resolution overrides** — `stub-paths` prepends user stub directories
  (resolution step 1); `typeshed-path` supplies your own typeshed tree, becoming
  the canonical step-3 source for standard-library types and disabling every
  other step-3 source, matching the pinned typing specification's "canonical source"
  clause
  ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst),
  [STUBRES-CUSTOM-TYPESHED](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED)).
  `typeshed-commit` pins an exact 40-character SHA and fails closed — an
  abbreviated SHA is rejected and another commit is never substituted;
  `typeshed-url` is a `{sha}` archive-mirror template, which cannot resolve
  Latest; `typeshed-cache-path` relocates automatic storage; `typeshed-cache`
  (default `true`) reuses re-hashed downloaded ZIP bytes, which expire after 24
  hours; `typeshed-verify` (default `true`) attests content against the trusted
  git tree, reporting `UNVERIFIED` when disabled without ever waiving the
  safety, shape, or license gates. Unpinned acquisition verifies `main` each run
  or session and never substitutes an older cached commit; the pin identity
  itself never expires
  ([STUBRES-TYPESHED-CONFIG](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-CONFIG)).
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
