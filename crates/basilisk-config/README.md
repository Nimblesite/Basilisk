# basilisk-config

Configuration parsing for Basilisk — reads `pyproject.toml` and `basilisk.json`.

## Role in Basilisk

This crate handles all **configuration discovery and parsing**. It reads project settings from `pyproject.toml` (under `[tool.basilisk]`) and `basilisk.json`, providing a unified config API to the checker and LSP.

## Key concepts

- **Per-path overrides** — allows relaxing strictness for legacy directories with optional deadlines.
- **Migration support** — reads existing `pyrightconfig.json` and `mypy.ini` to ease adoption.
- **Analysis modes** — configures workspace-level analysis behavior (single-file, workspace, etc.).

## Dependencies

| Crate | Purpose |
|-------|---------|
| `serde` | Deserialization |
| `serde_json` | JSON parsing |
| `toml` | TOML parsing |

## Status

Complete — consumed by `basilisk-checker`, `basilisk-cli`, and `basilisk-lsp`.
