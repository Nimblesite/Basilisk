# Formatting & Import Hygiene — Implementation Plan {#LSPFMT-PLAN}

Implements [LSPFMT](../specs/LSP-FORMATTING-SPEC.md#LSPFMT). Goal: jettison the `ruff` CLI, embed the Ruff formatter crate in-process, reimplement import hygiene natively, and make the formatter's Ruff provenance/version visible everywhere. Ratchets and CLAUDE.md rules apply throughout (coverage/mutation up, benches down, no `unwrap`/`panic`, structured logging).

## Phase 1 — Embed the Ruff formatter ([LSPFMT-ENGINE], [LSPFMT-CAPABILITIES])

1. Spike: add `ruff_python_formatter` to `Cargo.toml` at the **same pinned rev** as `ruff_python_parser` (`7c645a9…`), confirm it builds, and confirm the entry point/options type (`format_module_source`-style API). Measure binary-size delta.
2. Rewrite `crates/basilisk-lsp/src/formatting.rs` to call the crate in-process — **delete the `Command::new("ruff")` subprocess entirely**. The `ruff` binary is no longer a dependency of any kind. Failing test first: formatting produces correct output on a machine with **no `ruff` binary installed at all** (proving the subprocess is gone, replacing today's silent-`None` no-op).
3. Feed `[tool.ruff.format]` options (line length, quote/indent style, magic trailing comma) from `WorkspaceConfig` into the engine; add the fields to `WorkspaceConfig` (`crates/basilisk-lsp/src/config.rs`) and the loaders (`pyproject.toml`).
4. Advertise `document_range_formatting_provider` in `crates/basilisk-lsp/src/server/init.rs` and add a `rangeFormatting` handler (`features.rs`). Whole-doc + range share the engine. (On-type is a later, optional follow-up.)

## Phase 2 — Native import hygiene ([LSPFMT-IMPORTS])

1. Reimplement the three fixers on the Ruff AST in `crates/basilisk-lsp/src/code_actions/imports.rs`: organize (isort semantics), expand-wildcard, split-multi-import. Delete `run_ruff_fix` and the temp-file/`ruff check` path.
2. Parity tests vs. the Ruff fixers on representative fixtures (the existing `ws_test_code_actions.rs` `ruff`-gated tests become unconditional — native, always available).

## Phase 3 — Config & provenance ([LSPFMT-CONFIG], [LSPFMT-PROVENANCE])

1. Add the `basilisk.formatter` enum (`"ruff"` default / `"none"`; reserve `"basilisk"`). **Done for VS Code:** the two `basilisk.ruff.*` settings and the `readRuffSettings()` plumbing (`vscode-extension/src/lsp-client.ts`) are already deleted and replaced by `basilisk.formatter` in `package.json`, both READMEs, and the forwarded `initializationOptions`. Remaining: wire the flag through `WorkspaceConfig` (Rust), Zed config, and `basilisk.nvim` defaults so the server honours `"none"`.
2. Expose the embedded Ruff formatter version: compile-time constant from the pinned rev → `basilisk --version`, LSP `serverInfo.version`, and an Output-channel log line on each format.
3. Ship Ruff's MIT license in `THIRD-PARTY-LICENSES`/NOTICE (already owed for the parser crates).

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
