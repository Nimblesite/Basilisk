---
layout: layouts/docs.njk
title: Introduction
description: What Basilisk is, why it exists, and how it differs from every other Python type checker.
keywords: basilisk, python, type checker, static analysis, strict, rust
eleventyNavigation:
  key: Introduction
  order: 1
---

# Introduction

Basilisk is a static type analyzer for Python that enforces **complete type safety by default**. There is no gradual mode. There is no `--strict` flag you need to remember to pass. There is only one mode: strict.

If your Python function has an untyped parameter, Basilisk flags it. If your return type is missing, Basilisk flags it. If you use `Any` without an explicit annotation, Basilisk flags it. Every time. With no configuration required.

## The problem Basilisk solves

Python's type annotation syntax has existed since PEP 484 in 2015. Over the following decade, the ecosystem built increasingly sophisticated type checkers — Pyright, mypy, ty, Pyrefly — all capable of finding real bugs when pointed at fully-typed code.

The catch: every one of them defaults to *gradual typing*. Untyped code passes silently. `Any` spreads through type inference without warning. Strictness is something you must deliberately opt into, configure, remember to enforce in CI, and re-explain to every new team member.

The result: 88% of Python developers use type hints "always" or "often" — yet nearly 30% of those developers have no type checking in their CI pipeline ([Meta/Microsoft Python Typing Survey 2024](https://engineering.fb.com/2024/12/09/developer-tools/typed-python-2024-survey-meta/)).

Basilisk takes a different position. **Type annotations are contracts, not documentation.** A function without a return type annotation is not "partially typed" — it is untyped, and that is an error.

## What Basilisk is

- A **static type analyzer** that runs on `.py` files — no Python interpreter, no execution
- A **language server** (LSP) that brings real-time type checking to every editor
- A **CLI tool** for CI integration — exits with code 1 when errors are found
- A **migration assistant** that reads your existing `pyrightconfig.json` or `mypy.ini`
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

Basilisk is currently at **v0.1.0** — Phase 1 of a seven-phase roadmap. The core checker, name resolver, parser, and CLI are complete. All E0001–E0025 diagnostic rules are implemented and passing.

| Phase | Milestone | Status |
|---|---|---|
| 1 | Parser, resolver, type checker, CLI | Complete |
| 2 | LSP server, VS Code extension | In progress |
| 3 | All E0001–E0025 rules, 80% PEP coverage, migration mode | Planned |
| 4 | Mojo safety annotations (ownership, immutability, coercion) | Planned |
| 5 | WASM plugins, Django/Pydantic/SQLAlchemy | Planned |
| 6 | 95%+ PEP, SARIF/JUnit, enterprise hardening | Planned |
| 7 | Plugin marketplace, community stubs, ecosystem | Planned |

## Next steps

- [Install Basilisk](/docs/installation/) — build from source or install via cargo
- [Quick Start](/docs/quick-start/) — your first type check in under 5 minutes
- [All Rules](/docs/rules/) — browse every BSK-E and BSK-W diagnostic code
