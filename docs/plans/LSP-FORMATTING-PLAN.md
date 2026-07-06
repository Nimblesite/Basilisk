# Formatting & Import Hygiene — Implementation Plan {#LSPFMT-PLAN}

Implements [LSPFMT](../specs/LSP-FORMATTING-SPEC.md#LSPFMT). Goal: jettison the `ruff` CLI, embed the Ruff formatter crate in-process, reimplement import hygiene natively, and make the formatter's Ruff provenance/version visible everywhere. Ratchets and CLAUDE.md rules apply throughout (coverage/mutation up, benches down, no `unwrap`/`panic`, structured logging).

## Phase 1 — Embed the Ruff formatter ([LSPFMT-ENGINE], [LSPFMT-CAPABILITIES]) — **DONE** (#254)

1. ~~Spike~~ `ruff_python_formatter` + `ruff_formatter` + `ruff_python_stdlib` added at the same pinned rev; `format_module_source`/`format_range` confirmed.
2. ~~Rewrite~~ `crates/basilisk-lsp/src/formatting.rs` calls the crate in-process; the `Command::new("ruff")` subprocess is deleted. Failing-test-first e2e: `crates/basilisk-cli/tests/e2e_lsp_no_ruff.rs` drives the real binary over LSP stdio with an empty `PATH`. A live parity e2e (`ws_test_formatting.rs`) asserts byte-identical output vs `ruff format` where the binary exists.
3. ~~Options~~ `FormatStyle` on `WorkspaceConfig` (`config.rs::load_format_style`) reads `[tool.ruff] line-length` + `[tool.ruff.format]` quote/indent style and magic trailing comma from `pyproject.toml`.
4. ~~Capabilities~~ `document_range_formatting_provider` advertised; `range_formatting` handler added; whole-doc + range share the engine. (On-type remains a later, optional follow-up.)

## Phase 2 — Native import hygiene ([LSPFMT-IMPORTS]) — **DONE** (#261)

1. ~~Reimplement~~ The three fixers live in `crates/basilisk-lsp/src/import_hygiene/` (organize with isort semantics in `sort.rs`, expand-wildcard in `wildcard.rs`, split-multi-import in `mod.rs`); `run_ruff_fix` and the temp-file/`ruff check` path are deleted. Note: ruff has **no** F403 autofix, so expand-wildcard's behavior is defined natively (names used but never bound, minus builtins).
2. ~~Parity tests~~ Fixer semantics were pinned against real `ruff check --fix` probes (0.15.17); the `ws_test_code_actions.rs` tests are unconditional and assert affirmatively; `e2e_lsp_no_ruff.rs` asserts exact organized output with no `ruff` on PATH.

## Phase 3 — Config & provenance ([LSPFMT-CONFIG], [LSPFMT-PROVENANCE]) — **DONE**

1. ~~Flag~~ `basilisk.formatter` honoured server-side: `FormatterEngine` on `WorkspaceConfig` (`basilisk.json` `"formatter"`, pyproject `formatter =`), overridden by `initializationOptions.formatter` (VS Code forwards it). `"none"` suppresses the formatting capabilities and the handlers answer null. Zed/Neovim reach it via `basilisk.json` in the workspace.
2. ~~Version~~ `EMBEDDED_RUFF_FORMATTER_VERSION` derived at compile time (`crates/basilisk-lsp/build.rs` verifies the declared rev→version pair against `Cargo.lock` — drift fails the build) → plain `basilisk --version` engine line (the `--json` Shipwright contract is unchanged), LSP `serverInfo.version`, and a `tracing` log line on each format.
3. ~~License~~ Ruff's MIT license shipped in `THIRD-PARTY-LICENSES` (covers parser + formatter + stdlib crates).

## Phase 4 — Release-notes exposure ([LSPFMT-RELEASE-NOTES])

Generate (never hand-type) a release-notes block listing every `shipwright.json` component + version **and** the embedded Ruff formatter version, so a reader can tell exactly which formatter bytes shipped. Drift-guarded like the other generated artifacts.

## Phase 5 — Per-client wiring ([LSPFMT-CLIENTS]) & CLI

1. VS Code: format-on-save wiring; one-time, dismissible opt-in prompt to set `editor.defaultFormatter` to Basilisk — **never** hijack an existing default.
2. Zed `"formatter": "language_server"`; Neovim `vim.lsp.buf.format({ name = "basilisk" })`.
3. `basilisk format [paths]` CLI subcommand using the same embedded engine.

## Phase 6 — Docs & governance

1. Website/docs: a Formatting section stating plainly the formatter **is the embedded Ruff formatter** (no separate install), mostly linking to the [Ruff formatter docs](https://docs.astral.sh/ruff/formatter/) rather than re-documenting behavior we don't own ([LSPFMT-HONESTY]).
2. Via reviewed PR, update the CLAUDE.md Architecture bullet ("Linting/formatting: Ruff CLI subprocess — not reimplemented") to reflect: formatting **embeds** the Ruff crate in-process; import hygiene is **reimplemented** natively; the `ruff` CLI is not a runtime dependency. (Governance rule: CLAUDE.md changes go through review, never silent capture.)

## Acceptance

- No `Command::new("ruff")` anywhere in shipping code; no `ruff` in dev-required PATH for formatting/imports.
- Formatting + Format Selection work in all four consumers with no `ruff` installed.
- `basilisk --version` and release notes both state the embedded Ruff formatter version.
- Formatter output byte-identical to `ruff format` at the pinned rev on the fixture corpus; import fixers at parity.
- Coverage/mutation ratchets up; bench gate green.
