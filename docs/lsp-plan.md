# LSP Implementation Plan

## Current State (Phase 2)

The VS Code extension uses the **subprocess approach**: it runs
`basilisk check --output json <file>` on every save/open, parses the JSON
array from stdout, and pushes `vscode.Diagnostic` objects into the Problems
panel. No LSP server is involved.

`basilisk-lsp` currently exposes `check_source(source: &str) -> Vec<String>`,
which runs the full checker pipeline on an in-memory string. This is used by
the extension indirectly via the CLI binary and is exercised by the LSP
integration tests.

## Why LSP Is Deferred

The subprocess approach is sufficient for Problems, hover-free diagnostics, and
CI integration. LSP adds considerable complexity (async I/O, JSON-RPC protocol,
incremental sync, capability negotiation) for features that are not yet
required.

## When to Implement LSP

Implement LSP when any of the following is needed:

- **Hover**: show type information on hover
- **Go-to-definition**: navigate to type definitions
- **Code actions**: quick-fixes (e.g. add missing annotation)
- **Inline diagnostics without save**: publish diagnostics as the user types
- **Multi-root workspaces**: efficient per-workspace state

## Planned Implementation

### Crate

`crates/basilisk-lsp` — add `tower-lsp` or `lsp-server` dependency.

### Protocol subset (Phase 3 target)

| Method | Purpose |
|---|---|
| `initialize` | Capability negotiation |
| `textDocument/didOpen` | Parse + check on open |
| `textDocument/didChange` | Incremental re-check |
| `textDocument/didSave` | Full re-check on save |
| `textDocument/publishDiagnostics` | Push errors to client |
| `textDocument/hover` | Show inferred type |

### Extension change

Replace `execFile` + JSON parsing in `extension.ts` with
`vscode-languageclient` wiring to the `basilisk lsp` subcommand.

### Binary

Add `basilisk lsp` subcommand that starts the JSON-RPC server on stdio.
