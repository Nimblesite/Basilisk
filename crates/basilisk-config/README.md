# basilisk-config

Configuration parsing for Basilisk — reads `[tool.basilisk]` from
`pyproject.toml`, the only configuration source.

## Role in Basilisk

This crate owns the checker-facing `BasiliskConfig` and parses the two flat
rule maps the configuration model defines — `[tool.basilisk.rules]` and
`[tool.basilisk.rule-tags]` — plus the documented scalar keys
([CHKARCH-CONFIG-MODEL]). Discovery walks **up** from each checked file: every
ancestor `pyproject.toml` carrying a `[tool.basilisk]` table contributes.
Rule entries are never merged — the tables are kept as a nearest-first chain
and the nearest table that decides a rule wins outright; non-rule scalar
fields merge additively, nearest directory winning per key
([CHKARCH-CONFIG-DISCOVERY]). A stray legacy `basilisk.json` is never read and
is wholly inert — the configuration editor does not surface it at all
([LSPCFGED-CONTRACT] excludes shadowed-source reporting, and tests assert its
absence). LSP/editor settings such as analysis mode live in `basilisk-lsp`
today and are not parsed by this crate.

## Key concepts

- **Rule entries** — `[tool.basilisk.rules]` grades one rule code `error`,
  `warning`, `info`, or `disabled`; a `pep`-tagged rule may be graded down but
  never `disabled` — that resolution is invalid and fails the run before
  checking starts (`basilisk_checker::pep_disable_violations`).
  **Tag entries** — `[tool.basilisk.rule-tags]`
  grades every rule carrying a tag in one line; within a table a rule entry
  beats tag entries, and the strictest matching tag entry wins.
- **Folder scoping** — a rule is graded differently for part of the tree by
  placing a `pyproject.toml` with its own `[tool.basilisk]` table in that
  folder. There are no glob-path or per-module override tables.
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
- **Adoption target** — `basilisk adopt` records current error debt as ordinary
  warning-severity `[tool.basilisk.rules]` entries in the config file of the
  nearest folder governing each affected file. The adoption state *is* that set
  of warning entries: `unadopt` deletes them and re-running `adopt` recomputes
  them. There are no exact-file overrides, ownership markers, or adoption
  sidecar.

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
Validated mutation, ancestor-walk nearest-first discovery, content revisions, and
the editor API are implemented; the editor targets `pyproject.toml` only and
never reports a stray `basilisk.json` at all. Remaining
provenance, document-version safety, and domain consolidation work is tracked
in
[`LSP-CONFIGURATION-EDITOR-PLAN.md`](../../docs/plans/LSP-CONFIGURATION-EDITOR-PLAN.md).
