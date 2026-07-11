---
layout: layouts/docs.njk
title: "Basilisk: The Only 100% PEP-Conformant Python Language Server"
description: "The only Python type checker scoring 100% on the official python/typing conformance suite — and the fastest we've measured. Complete open-source Python dev environment in Rust: type checker, language server, debugger, profiler, plus VS Code, Cursor, Zed & Neovim extensions. Strict by default."
keywords: basilisk, python, language server, lsp, type checker, vs code, cursor, zed, neovim, strict, rust
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Introduction
  order: 1
---

# Introduction

Basilisk is a **complete, open-source Python language server**. Everything you rely on a modern Python extension for — autocomplete, go-to-definition, hover information, refactoring, diagnostics, integrated debugging, profiling — Basilisk does too, fully open source and conformant to the Python typing spec by default.

It is also the **only Python type checker with a perfect 100% score** on the [official `python/typing` conformance results]({{ conformanceOfficial.snapshot.source }}) — published on the Python typing repository's own leaderboard, ahead of Pyright, mypy, Pyrefly and ty. See [how we measure it](/docs/conformance/).

It is not just a type checker. It is a feature-complete LSP with first-class extensions for **VS Code**, **Zed**, and **Neovim** — plus any other editor that speaks the Language Server Protocol. **Cursor** and **Windsurf** (via Open VSX) are coming very soon, and JetBrains is on the way. No proprietary extension, no Node.js — a single Rust binary, the same experience in every editor.

## The problem Basilisk solves

[Pylance](https://marketplace.visualstudio.com/items?itemName=ms-python.vscode-pylance) is the default Python language extension in VS Code. It is also **proprietary** — you cannot inspect, modify, or redistribute it. Pyright, the open-source type checker underneath, is powerful but is *only* a type checker — it does not provide completions, hover, go-to-definition, or refactoring without the proprietary Pylance wrapper.

Every other Python type checker (mypy, ty, Pyrefly) is *only* a checker — no completions, no refactoring, no debugger. You assemble a language server separately and keep the two in step across the team.

Basilisk takes a different position. Its default *is* the typing spec — full PEP conformance out of the box — and it brings the whole stack (type checking, language features, debugging, profiling) into a single open-source tool that runs the same in **every** editor, not just VS Code. Want checking stricter than the spec? Switch on the opt-in Basilisk rules. Type annotations are contracts, not documentation.

## What Basilisk is

- A **full-featured language server** (LSP) — autocomplete, go-to-definition, hover, find references, rename, a full [refactoring suite](/docs/refactoring/), code actions, inlay hints
- **Editor extensions for every major IDE** — VS Code, Neovim (0.10+), and Zed today; Cursor and Windsurf (via Open VSX) coming very soon, and JetBrains (IntelliJ / PyCharm) on the way
- **Annotation quick-fixes** — one-click code actions that insert a placeholder annotation (`: Any`, `-> None`) on unannotated code, so you can fill in the real type
- An **integrated debugger** — press F5 to debug Python with breakpoints, stepping, variable inspection, and watch expressions, all brokered through the Basilisk LSP
- An **integrated profiler** — sampling CPU profiler with inline heatmap annotations, flame graphs, memory leak detection, and reference graph visualization, all inside your editor
- A **PEP-conformant type checker by default** — the core spec rule set out of the box, with opt-in Basilisk rules for checking stricter than the spec
- A **CLI tool** for CI integration — exits with code 1 when errors are found
- A **migration assistant** that reads your existing `pyrightconfig.json` or `mypy.ini`
- **uv integration** — workspace detection, lock file parsing, and package management commands
- Written in **Rust** — ships as a single binary with no runtime dependencies

![Basilisk activity panel in VS Code — Module Explorer with typed-coverage percentage, Python Processes for CPU and memory profiling, and type-checking status](/assets/images/vscode-module-explorer.png)

*The Basilisk activity panel: module type-coverage, one-click CPU/memory profiling, and live server status.*

## What Basilisk is not

- Not a compiler — your Python code runs on CPython as normal
- Not a runtime type checker — analysis happens statically at development time
- Not tied to one editor — the same server powers VS Code, Cursor, Windsurf, Zed, and Neovim

## Conformant by default, configurable from there

Basilisk's behaviour is decided entirely by **configuration**, and the default configuration is exactly the **core PEP conformance rule set** — the same rules the official typing-conformance suite grades. Out of the box you get a checker that follows the spec, with no flags to remember.

Stricter-than-spec checking is **opt-in**. Basilisk also ships extra rules the spec doesn't define — *require an annotation* on every parameter and return, a redundant-annotation warning, a missing-`@override` nudge, an explicit-`Any` nudge. They stay **off** until you enable them in config. Because they flag code the spec considers valid, turning them on deliberately trades strict spec conformance for a stricter standard of your team's choosing — a per-project choice, never a default.

Configuration is also where you relax rules for the paths that need it — for example, softening or disabling a rule across a legacy directory:

```toml
[tool.basilisk.per-path-overrides."legacy/**"]
disabled = ["returns_compatibility"]        # turn a rule off for legacy code
rules."imports_unresolved" = "warning"   # or just soften its severity
```

This keeps the default honest — pure spec conformance — while letting each team dial strictness exactly where they want it.

## Project status

Basilisk is under **active development** — the core checker, LSP server, and editor extensions are all working, and it is the only checker with a perfect score on the official python/typing conformance suite. Autocomplete, go-to-definition, hover, diagnostics, inlay hints, refactoring, debugging, and profiling are shipping today.

| Phase | Milestone | Status |
|---|---|---|
| 1 | Parser, resolver, type checker, CLI | Complete |
| 2 | LSP server, editor extensions (VS Code, Cursor, Zed, Neovim) | Complete |
| 3 | Expanded rule set, PEP conformance ({{ conformance.scorePct }}% on the pinned suite), gradual adoption | In progress |
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
