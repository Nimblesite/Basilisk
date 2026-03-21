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
- **Parallel file checking** — uses Rayon for work-stealing parallelism across files.
- **Exit codes** — `0` (clean), `1` (type errors found), `3` (internal error).
- **Output formats** — human-readable rustc-style (default) and JSON for editor/CI integration.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `basilisk-parser` | Parsing |
| `basilisk-resolver` | Name resolution |
| `basilisk-checker` | Type checking |
| `basilisk-config` | Configuration |
| `basilisk-stubs` | Type stubs |
| `clap` | CLI argument parsing |
| `rayon` | Parallel file processing |

## Status

Complete — stable binary published as `basilisk`.
