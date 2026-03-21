# basilisk-uv

uv package manager integration for the Basilisk LSP.

## Role in Basilisk

This crate provides **uv workspace detection and package intelligence** for the LSP. It parses `uv.lock` files, detects uv workspaces, and exposes package metadata so the LSP can offer commands like `uv sync` and `uv add` directly from the editor.

## Key concepts

- **Workspace detection** — discovers uv workspaces by walking up the directory tree for `pyproject.toml` files with `[tool.uv]` sections.
- **Lock file parsing** — reads `uv.lock` to understand installed packages and their versions.
- **LSP commands** — powers the `Basilisk: uv sync` and `Basilisk: uv add` editor commands.

## Status

Working — consumed by `basilisk-lsp`.
