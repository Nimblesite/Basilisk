---
layout: layouts/docs.njk
title: Introduction
description: What Basilisk is, why it exists, and how it replaces Pylance and Pyright as a fully open-source Python language server.
keywords: basilisk, python, language server, pylance replacement, pyright, type checker, lsp, vs code, strict, rust
eleventyNavigation:
  key: Introduction
  order: 1
---

# Introduction

![PLACEHOLDER: Basilisk VS Code extension showing diagnostics, autocomplete, and type checking in a Python file](intro-vscode-overview.png)

Basilisk is a **complete Python language server** that replaces Pylance and Pyright. Everything Pylance does — autocomplete, go-to-definition, hover information, refactoring, diagnostics, integrated debugging, profiling — Basilisk does too, fully open source and strict by default.

It is not just a type checker. It is a feature-complete LSP with first-class extensions for **VS Code**, **Neovim**, and **Zed** — plus any other editor that speaks the Language Server Protocol. No proprietary extensions. No Node.js. A single Rust binary.

## The problem Basilisk solves

Pylance is the most-used Python extension in VS Code. It is also **proprietary** — you cannot inspect, modify, or redistribute it. Pyright, the open-source type checker underneath, is powerful but is *only* a type checker — it does not provide completions, hover, go-to-definition, or refactoring without the proprietary Pylance wrapper.

Every other Python type checker (mypy, ty, Pyrefly) defaults to *gradual typing*. Untyped code passes silently. `Any` spreads through type inference without warning. Strictness is something you must deliberately opt into, configure, remember to enforce in CI, and re-explain to every new team member.

Basilisk takes a different position. **It replaces the entire Pylance stack** — type checking, language features, and VS Code integration — with an open-source alternative that is strict by default. Type annotations are contracts, not documentation.

## What Basilisk is

![PLACEHOLDER: Split view showing Basilisk running in VS Code, Neovim, and Zed side by side](intro-editor-support.png)

- A **full-featured language server** (LSP) — autocomplete, go-to-definition, hover, find references, rename, [16 refactoring actions](/docs/refactoring/), code actions, inlay hints
- **Editor extensions** for VS Code, Neovim (0.10+), and Zed — install it, disable Pylance, and everything works
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
- Not a Mojo dependency — Basilisk's ownership annotations work with standard Python today

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

## Mojo-inspired safety

Basilisk adds Mojo-inspired ownership semantics as static analysis annotations over standard Python syntax. Using `Annotated` from the `typing` module, you can declare that a parameter is:

- **`Borrowed`** — a read-only reference; mutation is a type error
- **`InOut`** — a mutable reference; must be explicitly declared
- **`Owned`** — ownership is transferred; use after transfer is a type error

These are not runtime constructs. They are statically checked annotations. Code that passes Basilisk's ownership checks is structurally compatible with Mojo's type expectations.

## Project status

![PLACEHOLDER: Basilisk project status dashboard showing phase completion and feature checklist](intro-project-status.png)

Basilisk is currently at **v0.1.0** — the core checker, LSP server, and VS Code extension are all working. Autocomplete, go-to-definition, hover, diagnostics, and inlay hints are shipping today.

| Phase | Milestone | Status |
|---|---|---|
| 1 | Parser, resolver, type checker, CLI | Complete |
| 2 | LSP server, VS Code extension | Complete |
| 3 | All E0001–E0025 rules, 80% PEP coverage, migration mode | In progress |
| 4 | Mojo safety annotations (ownership, immutability, coercion) | Planned |
| 5 | WASM plugins, Django/Pydantic/SQLAlchemy | Planned |
| 6 | 95%+ PEP, SARIF/JUnit, enterprise hardening | Planned |
| 7 | Plugin marketplace, community stubs, ecosystem | Planned |

## Architecture

Basilisk is a Cargo workspace with 14 Rust crates, each owning one layer of the system:

| Layer | Crates |
|-------|--------|
| **Analysis pipeline** | `basilisk-parser` &rarr; `basilisk-resolver` &rarr; `basilisk-checker` &rarr; `basilisk-cli` |
| **LSP & infrastructure** | `basilisk-lsp`, `basilisk-db`, `basilisk-config`, `basilisk-stubs`, `basilisk-uv`, `basilisk-common` |
| **Editor extensions** | VS Code (`vscode-extension`), Neovim (`basilisk.nvim`), Zed (`basilisk-zed`) |
| **Future** | `basilisk-mojo` (ownership), `basilisk-compiler` (native), `basilisk-plugin` (WASM plugins) |

## Next steps

- [Install Basilisk](/docs/installation/) — build from source or install via cargo
- [Quick Start](/docs/quick-start/) — your first type check in under 5 minutes
- [Refactoring](/docs/refactoring/) — all 16 refactoring code actions (extract, inline, move, rename, convert)
- [Debugging](/docs/debugging/) — set breakpoints, step through code, inspect variables
- [Profiler](/docs/profiler/) — CPU heatmaps, flamegraphs, memory leak detection, and reference graphs
- [All Rules](/docs/rules/) — browse every BSK-E and BSK-W diagnostic code
