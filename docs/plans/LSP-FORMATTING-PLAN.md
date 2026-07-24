# Formatting Follow-ups {#LSPFMT-PLAN}

Implements [LSPFMT](../specs/LSP-FORMATTING-SPEC.md#LSPFMT).

The embedded Ruff formatter, native import hygiene, range formatting,
`basilisk.formatter`, provenance/version reporting, no-`ruff` regression
coverage, `basilisk format`, generated release-notes provenance, and the
formatting docs page are complete. The `ruff` CLI is not a runtime
dependency. Open: the VS Code default-formatter opt-in prompt and the two
published-artifact verifications, which need a live editor/release.

## Release provenance {#LSPFMT-PLAN-RELEASE}

- [x] Generate a release-note component block from `shipwright.json` plus
  `EMBEDDED_RUFF_FORMATTER_VERSION` (`scripts/gen_release_notes.py`, appended
  to the auto-generated notes by the `release` job).
- [x] Drift-test the generated block so release notes cannot claim different
  formatter bytes from the build
  (`crates/basilisk-cli/tests/e2e_release_notes_block.rs` runs the generator
  against the freshly built binary).

## Client wiring {#LSPFMT-PLAN-CLIENTS}

- [ ] VS Code: offer a one-time, dismissible opt-in to set Basilisk as the Python
  default formatter only when the user has not chosen another formatter.
- [ ] Zed: verify the published extension selects the language-server formatter.
- [x] Neovim: document and test `vim.lsp.buf.format({ name = "basilisk" })`
  (`:h basilisk-formatting` in `doc/basilisk.txt`; real-LSP format call in
  `tests/lsp/ui_spec.lua`).

## CLI {#LSPFMT-PLAN-CLI}

- [x] Add `basilisk format [paths]` using the same embedded engine and project
  Ruff-format options as the LSP (`crates/basilisk-cli/src/format.rs`, plus
  `--check`).
- [x] Cover check/write behavior, multiple paths, parse failures, and formatter
  disablement with real-binary tests
  (`crates/basilisk-cli/tests/e2e_format.rs`, run with an empty `PATH`).

## Documentation {#LSPFMT-PLAN-DOCS}

- [x] Document that formatting embeds Ruff while import hygiene is Basilisk's
  native implementation; link to Ruff for formatter behavior rather than
  duplicating its manual (`website/src/docs/formatting.md`,
  `:h basilisk-formatting`).
- [ ] Verify formatting and import actions in published VS Code, Zed, and Neovim
  artifacts with no `ruff` executable on `PATH`.

## Acceptance {#LSPFMT-PLAN-ACCEPTANCE}

- Release notes identify every shipped binary and the embedded Ruff version.
- All three editors format without a `ruff` installation and preserve existing
  user formatter choices.
- `basilisk format` and LSP formatting are byte-identical for the same input and
  configuration.
