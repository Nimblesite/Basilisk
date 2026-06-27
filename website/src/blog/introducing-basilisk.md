---
layout: layouts/blog.njk
title: "Introducing Basilisk: Python's Type System, Actually Enforced"
description: "Python has had type annotations for ten years. It's time someone enforced them by default."
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
image: /assets/images/blog/introducing-basilisk.png
imageAlt: "Abstract Basilisk type-checking pipeline with validation nodes and strict analysis panels"
imageWidth: 1200
imageHeight: 675
tags: posts
category: announcements
excerpt: "The Python type annotation ecosystem has a dirty secret: nobody enforces it by default. We built Basilisk to fix that."
keywords: basilisk, python type checker, strict typing, rust, announcement
---

The Python type annotation ecosystem has a dirty secret: nobody enforces it by default.

PEP 484 landed in 2015. In the decade since, the ecosystem built sophisticated tools — Pyright, mypy, ty, Pyrefly — capable of catching real bugs when pointed at fully-typed code. The Python Typing Council published spec after spec. The `typing` module grew to cover generics, protocols, TypeVarTuple, ParamSpec, TypeIs, and more.

And yet: 88% of Python developers use type hints "always" or "often" — but nearly 30% of those developers have no type checking in their CI pipeline at all ([Meta/Microsoft Python Typing Survey 2024](https://engineering.fb.com/2024/12/09/developer-tools/typed-python-2024-survey-meta/)). The most popular type checker in VS Code defaults to a mode where untyped functions pass silently.

Why? Because every tool treats strictness as optional.

## The TypeScript lesson

TypeScript didn't make JavaScript slower or more constrained. It made large JavaScript codebases maintainable. It achieved this not through clever engineering alone, but through a design philosophy: **types are the default**. You have to explicitly opt out of type checking, not opt into it.

The result is that TypeScript adoption follows a different curve than Python typing adoption. Once a team uses TypeScript at all, the entire codebase tends to be typed. There's no gradual drift back to untyped code because the tooling doesn't encourage it.

Python's typing tools took the opposite approach. [Four modes in Pyright](https://microsoft.github.io/pyright/#/configuration?id=type-check-diagnostics-settings): `off`, `basic`, `standard`, `strict`. The default is not `strict`. Most teams never change the default.

## What every other tool gets wrong

The problem isn't technical capability. Pyright, at [~99% PEP conformance](https://github.com/python/typing/blob/main/conformance/results/results.html), is genuinely excellent at finding type errors when configured correctly. The problem is the default.

When strictness is opt-in:
- New projects start without it because there's no immediate pressure to add it
- Existing projects never add it because the error count on day one is discouraging
- CI scripts omit the `--strict` flag because it wasn't there when the script was written
- New team members don't know to add it
- The flag gets dropped when deadlines approach

The result is a codebase that *appears* to be using type checking but is actually running in a mode that allows the majority of type errors to pass silently.

## Basilisk's position

Basilisk is strict by default. There is no permissive mode. There is no `--strict` flag to forget to pass. There is one mode: every parameter must be typed, every return type must be declared, `Any` must always be explicit.

This is not about making Python developers' lives harder. It's about making the safe path the default path. When strictness is the default, type coverage naturally increases as teams add new code. There's nothing to remember to turn on.

Adopting Basilisk on an existing codebase does require work — but it's work that surfaces real bugs. Every BSK-E0001 is a function where the type contract was never defined. Every match_exhaustiveness is a `match` statement where an unhandled case was silently ignored. The errors Basilisk reports are not false positives — they are places where the type system was not being used.

Looking further ahead, [Mojo](https://www.modular.com/mojo)'s ownership semantics and immutability model are an inspiration for a planned future direction in Basilisk — but that is on the roadmap, not in the current release.

## Why Rust

Basilisk is implemented in Rust and ships as a single binary with no runtime dependencies.

The alternative — implementing a Python type checker in Python — has a fundamental problem: it requires a Python interpreter to run. In a CI environment that might be running Docker images, GitHub Actions runners, or edge build systems, adding a Python runtime dependency just to check Python types is unnecessary overhead.

More importantly, Rust's ownership model and zero-cost abstractions make it possible to implement the incremental computation Basilisk needs. When you edit a single file, Basilisk re-checks only that file and the modules that import it — the affected analysis results — and the rest stays cached. No persistent daemon, no whole-project re-analysis on every keystroke.

The result: incremental type checking that doesn't require a persistent daemon and is designed to stay responsive as your codebase grows.

## What exists today

Basilisk (alpha) implements Phase 1 of a seven-phase roadmap.

**Working today:**
- Core parser, name resolver, and type checker
- All E0001–E0025 diagnostic rules
- CLI: `basilisk check [path]`
- rustc-style error output with location, caret, help, and documentation links
- Exit code 1 on errors for CI integration
- Recursive directory checking

**Working today:**
- Language Server Protocol (LSP) server — autocomplete, go-to-definition, hover, diagnostics, inlay hints, a suite of refactoring actions
- VS Code extension — bundles the correct binary per platform; publishing to [Open VSX](https://open-vsx.org) (for Cursor, Windsurf, and other VS Code-compatible editors) is coming very soon
- Neovim plugin (0.10+)
- Zed extension
- Integrated debugger (debugpy, press F5)
- Integrated profiler (py-spy, flamegraphs, memory leak detection)

**On the roadmap:**
- Phase 3: 80% PEP coverage, `basilisk migrate`, gradual adoption
- Phase 4: Mojo safety annotations (ownership, immutability, coercion)
- Phase 5: WASM plugin system, Django/Pydantic/SQLAlchemy plugins, auto-stub generation
- Phase 6: 95%+ PEP coverage, SARIF/JUnit output, enterprise hardening
- Phase 7: Plugin marketplace, community stubs, ecosystem

## Try it

```bash
git clone https://github.com/Nimblesite/Basilisk
cd basilisk
cargo build --release
./target/release/basilisk check examples/bad.py
```

If you want to see the diagnostics in action, the repository includes `examples/bad.py` (with intentional errors), `examples/good.py` (clean), and `examples/mixed.py` (realistic mixed case).

File issues on [GitHub](https://github.com/Nimblesite/Basilisk/issues). The specification is in `SPEC.md` if you want to understand the full design.

Python type annotations have been optional for ten years. It's time they weren't.
