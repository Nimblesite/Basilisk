# basilisk-lsp

> **A record, not a product claim.** Basilisk is unlisted and its type checker is
> inert ([WITHDRAWAL](../../docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL)).
> Nothing described below ships in anything a user can install: the `basilisk`
> binary analyses nothing, and the editor extensions carry no checker. This file
> is kept as an account of what was built, and nothing in it authorises
> rebuilding what it describes.

Language Server Protocol implementation for Basilisk.

## Role in Basilisk

This is the **editor integration layer**. It implements the LSP specification over `tower-lsp`, providing real-time diagnostics, hover information, go-to-definition, code actions, refactoring, and inlay hints to any LSP-compatible editor (VS Code, Neovim, Zed, Emacs).

```
Editor ⟷ [basilisk-lsp] ⟷ parser + resolver + checker
```

## Key concepts

- **Beyond diagnostics** — the server also implemented completions, hover, go-to-definition, find references, rename, code actions, and inlay hints.
- **Incremental analysis** — depends on `salsa` directly and drives the `BasiliskDatabase` re-exported by `basilisk-checker` (defined in `basilisk-db`), keeping one persistent database across the session so unchanged files are served from the memo ([CHKARCH-INCREMENTAL-SALSA](../../docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA)).
- **Integrated debugging** — spawns debugpy and brokers DAP connections so editors get F5-to-debug without separate extensions.
- **Integrated profiling** — embeds py-spy for performance profiling with heatmap visualization.
- **Embedded Ruff formatter** — links the `ruff_python_formatter` crate into the binary and reimplements import hygiene natively on the Ruff AST. The `ruff` CLI is not a runtime dependency and is never spawned ([LSPFMT-ENGINE](../../docs/specs/LSP-FORMATTING-SPEC.md#LSPFMT-ENGINE), [LSPFMT-IMPORTS](../../docs/specs/LSP-FORMATTING-SPEC.md#LSPFMT-IMPORTS)).
- **Code actions & refactoring** — extract function/variable, rename, move symbol, inline, and more.
- **uv integration** — detects uv workspaces, parses lock files, and provides package intelligence.

## Dependencies

Principal direct dependencies, as declared in `Cargo.toml`:

| Crate | Purpose |
|-------|---------|
| `basilisk-parser` | Parsing |
| `basilisk-resolver` | Name resolution |
| `basilisk-checker` | Type checking; also re-exports `BasiliskDatabase` and the salsa inputs |
| `basilisk-config` | Configuration |
| `basilisk-stubs` | Type stubs |
| `basilisk-typeshed-fetch` | Typeshed acquisition |
| `basilisk-uv` | uv package manager |
| `basilisk-profiler-protocol` | Profiler wire protocol |
| `salsa` | Incremental computation |
| `tower-lsp` | LSP transport |
| `ruff_python_formatter` | In-process formatting engine |

There is no direct dependency on `basilisk-db`; the Salsa database type it
defines reaches this crate through `basilisk-checker`'s re-export.

## Status

The language server ships in nothing. `basilisk lsp` no longer exists — the CLI
parses no arguments and starts no server — and no editor extension launches
one. Nothing described above runs for a user.
