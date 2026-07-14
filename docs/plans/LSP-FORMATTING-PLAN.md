# Formatting Follow-ups {#LSPFMT-PLAN}

Implements [LSPFMT](../specs/LSP-FORMATTING-SPEC.md#LSPFMT).

The embedded Ruff formatter, native import hygiene, range formatting,
`basilisk.formatter`, provenance/version reporting, and no-`ruff` regression
coverage are complete. The `ruff` CLI is not a runtime dependency.

## Release provenance {#LSPFMT-PLAN-RELEASE}

- [ ] Generate a release-note component block from `shipwright.json` plus
  `EMBEDDED_RUFF_FORMATTER_VERSION`.
- [ ] Drift-test the generated block so release notes cannot claim different
  formatter bytes from the build.

## Client wiring {#LSPFMT-PLAN-CLIENTS}

- [ ] VS Code: offer a one-time, dismissible opt-in to set Basilisk as the Python
  default formatter only when the user has not chosen another formatter.
- [ ] Zed: verify the published extension selects the language-server formatter.
- [ ] Neovim: document and test `vim.lsp.buf.format({ name = "basilisk" })`.

## CLI {#LSPFMT-PLAN-CLI}

- [ ] Add `basilisk format [paths]` using the same embedded engine and project
  Ruff-format options as the LSP.
- [ ] Cover check/write behavior, multiple paths, parse failures, and formatter
  disablement with real-binary tests.

## Documentation {#LSPFMT-PLAN-DOCS}

- [ ] Document that formatting embeds Ruff while import hygiene is Basilisk's
  native implementation; link to Ruff for formatter behavior rather than
  duplicating its manual.
- [ ] Verify formatting and import actions in published VS Code, Zed, and Neovim
  artifacts with no `ruff` executable on `PATH`.

## Acceptance {#LSPFMT-PLAN-ACCEPTANCE}

- Release notes identify every shipped binary and the embedded Ruff version.
- All three editors format without a `ruff` installation and preserve existing
  user formatter choices.
- `basilisk format` and LSP formatting are byte-identical for the same input and
  configuration.
