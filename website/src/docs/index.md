---
layout: layouts/docs.njk
title: "Basilisk: Open-Source Python Language Server"
description: "Basilisk is a strict-by-default Python language server with autocomplete, go-to-definition, refactoring, debugging, and profiling — fully open source, built in Rust, in VS Code, Cursor, Zed, and Neovim."
keywords: basilisk, python, language server, lsp, type checker, vs code, cursor, zed, neovim, strict, rust
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Introduction
  order: 1
---

# Introduction

Basilisk is a **complete, open-source Python language server**. Everything you rely on a modern Python extension for — autocomplete, go-to-definition, hover information, refactoring, diagnostics, integrated debugging, profiling — Basilisk does too, fully open source and strict by default.

It is not just a type checker. It is a feature-complete LSP with first-class extensions for **VS Code**, **Cursor**, **Windsurf**, **Zed**, and **Neovim** — plus any other editor that speaks the Language Server Protocol (JetBrains is on the way). No proprietary extension, no Node.js — a single Rust binary, the same experience in every editor.

## The problem Basilisk solves

[Pylance](https://marketplace.visualstudio.com/items?itemName=ms-python.vscode-pylance) is the default Python language extension in VS Code. It is also **proprietary** — you cannot inspect, modify, or redistribute it. Pyright, the open-source type checker underneath, is powerful but is *only* a type checker — it does not provide completions, hover, go-to-definition, or refactoring without the proprietary Pylance wrapper.

Every other Python type checker (mypy, ty, Pyrefly) defaults to *gradual typing*. Untyped code passes silently. `Any` spreads through type inference without warning. Strictness is something you must deliberately opt into, configure, remember to enforce in CI, and re-explain to every new team member.

Basilisk takes a different position. It brings the whole stack — type checking, language features, debugging, and profiling — into a single open-source tool that is strict by default and runs the same in **every** editor, not just VS Code. Type annotations are contracts, not documentation.

## What Basilisk is

- A **full-featured language server** (LSP) — autocomplete, go-to-definition, hover, find references, rename, a full [refactoring suite](/docs/refactoring/), code actions, inlay hints
- **Editor extensions for every major IDE** — VS Code, Cursor, and Windsurf (via Open VSX), plus Neovim (0.10+) and Zed; JetBrains (IntelliJ / PyCharm) is on the way
- **Enrichment fixes** — one-click code actions that add the missing type annotations *for* you
- An **integrated debugger** — press F5 to debug Python with breakpoints, stepping, variable inspection, and watch expressions, all brokered through the Basilisk LSP
- An **integrated profiler** — CPU profiling via py-spy with inline heatmap annotations, flamegraphs, memory leak detection, and reference graph visualization, all inside your editor
- A **strict-by-default type checker** — no `--strict` flag, no gradual mode, no opt-in
- A **CLI tool** for CI integration — exits with code 1 when errors are found
- A **migration assistant** that reads your existing `pyrightconfig.json` or `mypy.ini`
- **uv integration** — workspace detection, lock file parsing, and package management commands
- Written in **Rust** — ships as a single binary with no runtime dependencies

## What Basilisk is not

- Not a compiler — your Python code runs on CPython as normal
- Not a runtime type checker — analysis happens statically at development time
- Not tied to one editor — the same server powers VS Code, Cursor, Windsurf, Zed, and Neovim

## One mode only

Basilisk has a single operating mode. There is no `--basic`, `--standard`, or `--permissive` flag. This is intentional.

When strictness is opt-in, teams drift toward permissive defaults. Deadlines arrive. Technical debt accumulates. The `--strict` flag never gets added to the CI script. Basilisk removes that possibility entirely.

Opting out is still possible — for legacy directories, with per-path configuration and an optional deadline after which the relaxation expires:

```toml
[tool.basilisk.per-path-overrides."legacy/**"]
strict = false
deadline = "2026-12-31"
```

This acknowledges that large codebases cannot be fully typed overnight, while ensuring that the permissive period has an expiration date.

## Project status

Basilisk is currently in **alpha** — the core checker, LSP server, and editor extensions are all working. Autocomplete, go-to-definition, hover, diagnostics, inlay hints, refactoring, debugging, and profiling are shipping today.

| Phase | Milestone | Status |
|---|---|---|
| 1 | Parser, resolver, type checker, CLI | Complete |
| 2 | LSP server, editor extensions (VS Code, Cursor, Zed, Neovim) | Complete |
| 3 | Expanded rule set, 92.5% PEP conformance, migration mode | In progress |
| 4 | Ownership & immutability analysis (Mojo-inspired) | Planned |
| 5 | WASM plugins, Django/Pydantic/SQLAlchemy | Planned |
| 6 | 95%+ PEP, SARIF/JUnit, JetBrains extension | Planned |
| 7 | Plugin marketplace, community stubs, ecosystem | Planned |

## Architecture

Basilisk is a Cargo workspace with 16 Rust crates, each owning one layer of the system:

| Layer | Crates |
|-------|--------|
| **Analysis pipeline** | `basilisk-parser` &rarr; `basilisk-resolver` &rarr; `basilisk-checker` &rarr; `basilisk-cli` |
| **LSP & infrastructure** | `basilisk-lsp`, `basilisk-db`, `basilisk-config`, `basilisk-stubs`, `basilisk-uv`, `basilisk-common`, `basilisk-test-utils`, `basilisk-profiler-helper` |
| **Editor extensions** | VS Code (`vscode-extension`), Neovim (`basilisk.nvim`), Zed (`basilisk-zed`) |
| **Future** | `basilisk-mojo` (ownership), `basilisk-compiler` (native), `basilisk-plugin` (WASM plugins) |

## Next steps

- [Install Basilisk](/docs/installation/) — Homebrew, Scoop, your editor's marketplace, or build from source
- [Quick Start](/docs/quick-start/) — your first type check in under 5 minutes
- [Refactoring](/docs/refactoring/) — the full refactoring suite (extract, inline, move, rename, convert)
- [Debugging](/docs/debugging/) — set breakpoints, step through code, inspect variables
- [Profiler](/docs/profiler/) — CPU heatmaps, flamegraphs, memory leak detection, and reference graphs
- [All Rules](/docs/rules/) — browse every BSK-E and BSK-W diagnostic code
