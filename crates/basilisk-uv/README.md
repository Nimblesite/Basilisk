# basilisk-uv

> **A record, not a product claim.** Basilisk is unlisted and its type checker is
> inert ([WITHDRAWAL](../../docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL)).
> Nothing described below ships in anything a user can install: the `basilisk`
> binary analyses nothing, and the editor extensions carry no checker. This file
> is kept as an account of what was built, and nothing in it authorises
> rebuilding what it describes.

uv package manager integration for the Basilisk LSP.

## Role in Basilisk

This crate provides **uv workspace detection and package intelligence** for the LSP. It parses `uv.lock` files, detects uv workspaces, and exposes package metadata so the LSP can offer commands like `uv sync` and `uv add` directly from the editor.

## Key concepts

- **Workspace detection** — discovers uv workspaces by walking up the directory tree for `pyproject.toml` files with `[tool.uv]` sections.
- **Lock file parsing** — reads `uv.lock` to understand installed packages and their versions.
- **LSP commands** — powers the `Basilisk: uv sync` and `Basilisk: uv add` editor commands.

## Status

Consumed only by the language server, which ships in nothing.
