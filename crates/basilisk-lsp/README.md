# basilisk-lsp

Language Server Protocol implementation for Basilisk.

## Role in Basilisk

This is the **editor integration layer**. It implements the LSP specification over `tower-lsp`, providing real-time diagnostics, hover information, go-to-definition, code actions, refactoring, and inlay hints to any LSP-compatible editor (VS Code, Neovim, Zed, Emacs).

```
Editor ⟷ [basilisk-lsp] ⟷ parser + resolver + checker
```

## Key concepts

- **Full LSP** — not just a type checker. Provides completions, hover, go-to-definition, find references, rename, code actions, and inlay hints.
- **Incremental analysis** — integrates with `basilisk-db` (Salsa) for sub-10ms response times on edits.
- **Integrated debugging** — spawns debugpy and brokers DAP connections so editors get F5-to-debug without separate extensions.
- **Integrated profiling** — embeds py-spy for performance profiling with heatmap visualization.
- **Embedded Ruff formatter** — links the `ruff_python_formatter` crate into the binary and reimplements import hygiene natively on the Ruff AST. The `ruff` CLI is not a runtime dependency and is never spawned ([LSPFMT-ENGINE](../../docs/specs/LSP-FORMATTING-SPEC.md#LSPFMT-ENGINE), [LSPFMT-IMPORTS](../../docs/specs/LSP-FORMATTING-SPEC.md#LSPFMT-IMPORTS)).
- **Code actions & refactoring** — extract function/variable, rename, move symbol, inline, and more.
- **uv integration** — detects uv workspaces, parses lock files, and provides package intelligence.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `basilisk-parser` | Parsing |
| `basilisk-resolver` | Name resolution |
| `basilisk-checker` | Type checking |
| `basilisk-config` | Configuration |
| `basilisk-stubs` | Type stubs |
| `basilisk-db` | Incremental computation |
| `basilisk-uv` | uv package manager |
| `tower-lsp` | LSP transport |
| `ruff_python_formatter` | In-process formatting engine |

## Status

Working — diagnostics, hover, go-to-definition, code actions, inlay hints, debugging, and refactoring are all shipping.
