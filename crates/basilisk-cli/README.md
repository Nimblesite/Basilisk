# basilisk-cli

Command-line interface for Basilisk — the `basilisk` binary.

## Role in Basilisk

This is the **user-facing entry point** for command-line usage. It wires the full analysis pipeline together (parser, resolver, checker) and presents diagnostics in rustc-style output. Used directly by developers and in CI pipelines.

```sh
basilisk check src/           # check a directory
basilisk check app.py         # check a single file
basilisk check src/ --output json  # JSON output for tooling
```

## Key concepts

- **Pipeline orchestration** — calls `basilisk-parser` → `basilisk-resolver` → `basilisk-checker` in sequence for each file.
- **Analysis-sized stack** — every subcommand is dispatched through `basilisk_lsp::runtime::run_with_analysis_stack`, so the recursive resolver and checker cannot overflow the default main-thread stack on deeply nested expressions ([LSPARCH-ARCH-STACK](../../docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-STACK)). Collected files are then checked in a single sequential pass on that thread.
- **Exit codes** — `0` (completed without error diagnostics), `1` (error diagnostics were found), `2` (invalid configuration), `3` (internal failure). See [CHKARCH-CLI-EXITCODES](../../docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI-EXITCODES).
- **Output formats** — human-readable rustc-style (default) and JSON for editor/CI integration.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `basilisk-parser` | Parsing |
| `basilisk-resolver` | Name resolution |
| `basilisk-checker` | Type checking |
| `basilisk-config` | Configuration |
| `basilisk-stubs` | Type stubs |
| `basilisk-lsp` | LSP server (`basilisk lsp`) and the analysis-stack runtime |
| `clap` | CLI argument parsing |

## Status

Complete — stable binary published as `basilisk`.
