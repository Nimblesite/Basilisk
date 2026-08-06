---
layout: layouts/docs.njk
title: "Python Language Server & Type Checker — Basilisk Docs"
description: "Install, configure, and use Basilisk: an open-source Python type checker and language server in Rust, with refactoring, formatting, debugging, profiling, and editor integrations."
keywords: basilisk, python language server, python type checker, python typing, lsp, vs code, cursor, zed, neovim, rust
date: 2026-02-28
dateModified: 2026-08-06
author: The Basilisk Project
eleventyNavigation:
  key: Introduction
  order: 1
---

# Introduction

Basilisk is an open-source **Python type checker and language server** built in Rust. It adds code intelligence, formatting, type-aware refactoring, testing, debugging, and CPU and memory profiling to your editor. Its default rules are intended to implement the Python typing specification, and that implementation is currently undergoing an integrity review.

**Conformance correction:** Basilisk's former result is withdrawn, its current percentage is temporarily unknown, and it has been removed from the official `python/typing` results at our request. We are rebuilding affected logic from scratch and will publish a new result after robustness and mutation verification. Read the [full correction](/docs/conformance/).

Extensions ship for **VS Code**, **Cursor**, **Windsurf**, **Zed**, and **Neovim**; any editor that speaks the Language Server Protocol can use the same server. JetBrains support is planned. Feature coverage varies per editor — see [the integration matrix](/docs/installation/#integration-status-by-editor).

## Why Basilisk exists

[Pylance](https://marketplace.visualstudio.com/items?itemName=ms-python.vscode-pylance), the default Python extension in VS Code, is [proprietary](https://github.com/microsoft/pylance-release/blob/main/FAQ.md) — you cannot inspect, modify, or redistribute it. [Pyright](https://microsoft.github.io/pyright/#/features), the open-source checker underneath it, is a type checker only: completions, hover, go-to-definition, and refactoring come from the proprietary Pylance layer. mypy, ty, and Pyrefly are checkers too, so a full workflow means assembling a language server, a debugger, and a profiler alongside them, then keeping that stack in step across a team.

Basilisk puts type checking, language features, formatting, debugging, and profiling in one open-source binary. Typing-spec rules are enabled by default; checking stricter than the spec is available as opt-in rules you enable in configuration. Whether those default rules implement the specification correctly is the subject of the current audit.

## What Basilisk is

- A **language server** — autocomplete, go-to-definition, hover, find references, rename, [refactoring](/docs/refactoring/), code actions, and inlay hints
- **Editor extensions** — VS Code, Cursor, Windsurf, Neovim (0.11+), and Zed today; JetBrains (IntelliJ / PyCharm) is planned
- **Annotation quick-fixes** — code actions that insert a placeholder annotation (`: Any`, `-> None`) on unannotated code for you to replace with the real type. They do not infer types
- An **integrated debugger** — press F5 to debug Python with breakpoints, stepping, variable inspection, and watch expressions, brokered by the Basilisk LSP. Requires `debugpy` in your project environment. See [Debugging](/docs/debugging/)
- An **integrated profiler** — sampling CPU profiler with inline heatmap annotations, flame graphs, memory leak detection, and reference graphs. See [Profiler](/docs/profiler/)
- A **built-in formatter** — the Ruff formatter compiled into the binary, plus native import organizing. See [Formatting](/docs/formatting/)
- A **type checker with PEP-derived rules enabled by default** — those rules run on `basilisk check`; opt-in Basilisk rules run on `basilisk analyze` once you enable them
- **Standard-library types with no setup** — a complete typeshed `stdlib/` tree is compiled into the binary and checking never downloads anything. Pin an exact `python/typeshed` commit, or a typeshed distribution by wheel SHA-256, and it is verified offline against your local store
- A **CLI for CI** — `basilisk check` exits 1 when errors are found; `basilisk format --check` exits 1 when a file would change
- **uv integration** — workspace detection, lock-file parsing, and package management commands
- An **MCP server** — `basilisk mcp` serves read-only typeshed source status over stdio to MCP clients
- Written in **Rust** — a single binary with no Node.js and no Python runtime needed to check or to run the language server

![Basilisk activity panel in VS Code — Module Explorer with typed-coverage percentage, Python Processes for CPU and memory profiling, and type-checking status](/assets/images/vscode-module-explorer.png)

*The Basilisk activity panel: module type-coverage, one-click CPU/memory profiling, and live server status.*

## What Basilisk is not

- Not a compiler — your Python code runs on CPython as normal
- Not a runtime type checker — analysis happens statically, at development time
- Not a Python runtime or package manager — running, testing, debugging, and memory profiling use your project's own interpreter
- Not tied to one editor — the same server backs VS Code, Cursor, Windsurf, Zed, and Neovim, though what each editor surfaces differs

## Typing-spec rules by default, configurable from there

Basilisk's behaviour is decided entirely by **configuration**, and the default configuration enables the **core PEP rule set** — the rules the official typing-conformance suite grades. These rules aim to follow the specification with no strictness flag required, but the withdrawn result means their actual conformance is temporarily unknown while they are audited and reimplemented where necessary.

Stricter-than-spec checking is **opt-in**. Basilisk also ships extra rules the spec doesn't define — *require an annotation* on every parameter and return, a redundant-annotation warning, a missing-`@override` nudge, an explicit-`Any` nudge. They stay **off** until you enable them in config. Because they flag code the spec considers valid, turning them on deliberately trades strict spec conformance for a stricter standard of your team's choosing — a per-project choice, never a default.

Configuration is also where you relax rules for the paths that need it — place a `pyproject.toml` with a `[tool.basilisk]` table in the folder, and the nearest deciding table wins for the files beneath it:

```toml
# legacy/pyproject.toml
[tool.basilisk.rules]
"returns_compatibility" = "warning"   # graded down for legacy code only
"imports_unresolved" = "info"
```

This keeps the default focused on spec-derived rules while letting each team dial additional strictness exactly where they want it.

## Project status

Basilisk is under **active development** — the core checker, LSP server, and editor extensions are working. Its former conformance result is withdrawn while affected checker logic is rebuilt and verified. Autocomplete, go-to-definition, hover, diagnostics, inlay hints, refactoring, debugging, and profiling are shipping today.

| Phase | Milestone | Status |
|---|---|---|
| 1 | Parser, resolver, type checker, CLI | Complete |
| 2 | LSP server, editor extensions (VS Code, Cursor, Zed, Neovim) | Complete |
| 3 | Clean PEP-rule reimplementation, robustness and mutation verification, gradual adoption | In progress |
| 4 | WASM plugins, Django/Pydantic/SQLAlchemy | Planned |
| 5 | SARIF/JUnit output, JetBrains extension | Planned |
| 6 | Plugin marketplace, community stubs, ecosystem | Planned |

## Architecture

Basilisk is a Cargo workspace with 18 Rust crates, each owning one layer of the system:

| Layer | Crates |
|-------|--------|
| **Analysis pipeline** | `basilisk-parser` &rarr; `basilisk-resolver` &rarr; `basilisk-checker` &rarr; `basilisk-cli` |
| **LSP & infrastructure** | `basilisk-lsp`, `basilisk-db`, `basilisk-config`, `basilisk-stubs`, `basilisk-uv`, `basilisk-common`, `basilisk-buildinfo`, `basilisk-profiler-helper`, `basilisk-profiler-protocol` |
| **Typeshed downloads** | `basilisk-typeshed-fetch` — the workspace's only HTTP client; it downloads typeshed on an explicit user action, strictly segregated from checking |
| **Test infrastructure** | `basilisk-test-utils`, `basilisk-test-macros` |
| **Editor extensions** | VS Code (`vscode-extension`), Neovim (`basilisk.nvim`), Zed (`basilisk-zed`) |

## Next steps

- [Install Basilisk](/docs/installation/) — PyPI (`uv tool install`), Homebrew, Scoop, your editor's marketplace, or build from source
- [Quick Start](/docs/quick-start/) — your first type check in under 5 minutes
- [Refactoring](/docs/refactoring/) — the full refactoring suite (extract, inline, move, rename, convert)
- [Debugging](/docs/debugging/) — set breakpoints, step through code, inspect variables
- [Profiler](/docs/profiler/) — CPU heatmaps, flamegraphs, memory leak detection, and reference graphs
- [All Rules](/docs/rules/) — browse every BSK-E and BSK-W diagnostic code
