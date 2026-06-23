---
layout: layouts/docs.njk
title: "Basilisk vs Pyright vs mypy vs ty vs Pyrefly"
description: "Detailed comparison of Python type checkers: Basilisk, Pyright, mypy, ty, and Pyrefly. Strictness, PEP conformance, performance benchmarks, and feature differences."
keywords: basilisk vs pyright, python type checker comparison, mypy vs basilisk, ty, pyrefly
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Comparison
  order: 6
---

# Comparing Type Checkers

The Python type checker landscape has changed significantly. Three new Rust-based tools launched in 2025. Every one of them defaults to gradual typing.

## The fundamental question

Before comparing features and performance, there is one question that determines whether a type checker can actually enforce type safety across a team:

**Does the tool flag untyped code by default?**

| Tool | Flags untyped code by default? |
|---|---|
| Basilisk | Yes |
| Pyright | No — must pass `--strict` or configure `typeCheckingMode = "strict"` |
| mypy | No — must pass `--strict` |
| ty | No |
| Pyrefly | No |

Every tool except Basilisk allows untyped code to pass silently in their default configuration. When strictness is opt-in, it tends not to happen. Teams under deadline pressure skip the flag. New projects never add it. CI scripts omit it.

Basilisk removes the choice. There is no permissive mode to fall back to.

---

## Full capability comparison

| Feature | Basilisk | Pyright | mypy | ty | Pyrefly |
|---|---|---|---|---|---|
| Strict by default | ✅ | ❌ opt-in | ❌ opt-in | ❌ opt-in | ❌ opt-in |
| PEP conformance¹ | 40.4% current (→100% target) | ~99% | ~58% | early alpha | ~86% |
| Implementation | Rust | TypeScript | Python/C | Rust | Rust |
| Runtime required | None | Node.js | Python | None | None |
| Full LSP (completions, hover, goto) | ✅ | Pylance only | ❌ | Basic | Basic |
| Integrated debugger | ✅ | ❌ | ❌ | ❌ | ❌ |
| Integrated profiler | ✅ | ❌ | ❌ | ❌ | ❌ |
| Editor extensions | VS Code, Zed, Neovim (Open VSX for Cursor/Windsurf soon) | Proprietary (Pylance) | None | VS Code | VS Code |
| Plugin system | WASM (planned) | None | Python hooks | Planned | None |
| License | MIT | MIT | MIT | MIT | MIT |

<a name="footnotes"></a>

**Sources:**

¹ Full-pass score from the [official python/typing conformance suite](https://github.com/python/typing/blob/main/conformance/results/results.html) (pyright 1.1.408, mypy 1.19.1, pyrefly 0.54.0). mypy's partial+pass score is 96.4%. ty is not yet included in the official suite — alpha-stage figure from [sinon.github.io/future-python-type-checkers](https://sinon.github.io/future-python-type-checkers/) (August 2025, alpha build).

---

## Pyright

**By Microsoft. TypeScript-based. ~99% PEP conformance ([source](https://github.com/python/typing/blob/main/conformance/results/results.html)).**

Pyright is the most conformant Python type checker available today. It correctly handles the vast majority of PEP typing features and has excellent performance for a TypeScript-based tool.

**What Pyright does well:**
- Highest PEP coverage of any shipping tool (~99% on the official conformance suite)
- Excellent documentation and error messages
- Deep VS Code integration via Pylance
- Fast enough for interactive use in most codebases
- Good inference for complex generics and protocols

**What Pyright doesn't do:**
- Strict by default — four modes: `off`, `basic`, `standard`, `strict`
- Requires Node.js to run — adds a dependency to Python-only CI environments
- Pylance (the VS Code extension) is proprietary — its richest features don't leave VS Code
- No plugins — no way to add framework-specific type intelligence

**When Pyright makes sense:** If you're already invested in the Microsoft VS Code ecosystem and don't mind the Node.js dependency, Pyright's current PEP conformance makes it the strongest choice for pure type checking today. Basilisk targets exceeding its conformance in Phase 3.

---

## mypy

**The original. Python/C-based. ~58% full-pass, 96% partial+pass ([source](https://github.com/python/typing/blob/main/conformance/results/results.html)).**

mypy defined what Python type checking looks like. Its `--strict` flag was the reference implementation for what "strict" means in Python typing for years.

**What mypy does well:**
- Established plugin ecosystem: Django, SQLAlchemy, Pydantic all have mypy plugins
- `--strict` flag is well-understood and documented
- Largest community and most StackOverflow answers
- Long history means most edge cases are handled

**What mypy doesn't do:**
- Significantly slower than Rust-based tools on large codebases
- Daemon mode (`dmypy`) is fragile under certain conditions
- Not strict by default
- Requires a Python runtime
- Plugin API is Python-only — no WASM portability

**When mypy makes sense:** Existing codebases with heavy investment in mypy plugins (Django, SQLAlchemy) may find migration effort significant until Basilisk's WASM plugin ecosystem reaches parity.

---

## ty (Astral)

**Built by the Ruff team. Rust + Salsa. Early alpha — not yet in the official conformance suite.**

ty is the most interesting new entrant. It's built by the same team that created Ruff (now the de facto Python linter), uses a Salsa-based incremental architecture, is built in Rust like Basilisk, and has Astral's engineering velocity behind it.

**What ty does well:**
- Rust-based incremental architecture (Salsa)
- Built by a team with a track record of shipping
- MIT licensed, fully open source
- Sub-10ms incremental speed ([4.7ms on PyTorch](https://astral.sh/blog/ty), December 2025)

**What ty doesn't do (yet):**
- Not yet included in the [official python/typing conformance suite](https://github.com/python/typing/blob/main/conformance/results/results.html) — still in early alpha
- Gradual typing by default
- No integrated debugger or profiler

**When ty makes sense:** If you want to bet on Astral's velocity and can tolerate lower type coverage during the adoption period. ty may eventually become a major player; it's too early to depend on it for strict enforcement.

---

## Pyrefly (Meta)

**Production-tested at Instagram scale. Rust-based. ~86% PEP conformance ([source](https://github.com/python/typing/blob/main/conformance/results/results.html)).**

Pyrefly was built by Meta to handle their Python codebase — one of the largest in the world. It emphasizes throughput ([1.85M LOC/sec on 166-core Meta infrastructure](https://pyrefly.org/)) over strict enforcement.

**What Pyrefly does well:**
- Battle-tested on millions of lines of production Python
- High throughput suitable for monorepo-scale codebases
- Rust-based, no runtime dependency
- Good documentation

**What Pyrefly doesn't do:**
- Strict by default — not available
- No plugin system
- Meta-driven roadmap — external contributions have less influence

**When Pyrefly makes sense:** Extremely large codebases (500K+ LOC) where throughput matters more than strict enforcement, particularly if the team has Meta-adjacent tooling.

---

## Basilisk's position

Basilisk is not a faster version of an existing tool. It occupies a different position:

**Unique to Basilisk:**
1. Strict by default — the only tool where you cannot accidentally run in permissive mode, yet you can dial rules down per-file or per-path from the editor UI or config
2. Enrichment fixes — one-click code actions that add the missing types *for* you, instead of just reporting that they're missing
3. A complete, open-source LSP in every editor — completions, hover, go-to-definition, refactoring, debugging, and profiling, the same in VS Code, plus native Zed and Neovim extensions (Open VSX for Cursor, Windsurf, and others coming very soon; JetBrains planned) — not just inside one proprietary VS Code extension
4. Integrated debugger and profiler brokered through the language server
5. WASM plugin system (planned) — extensible without forking, secure by design

**Where Basilisk is not yet the best choice:**
- PEP conformance: Basilisk currently passes 40.4% of the official conformance suite (59/146, counting errors+warnings — the strictest grading), with 285 false positives and 36 missed required errors still being driven down. Pyright covers far more edge cases today. Basilisk's target is 100%; it's not there yet.
- Plugin ecosystem: mypy's Django and SQLAlchemy plugins are mature. Basilisk's WASM plugins are planned.
- Maturity: Pylance is feature-complete today (though proprietary and VS Code only). Basilisk is in alpha.

The honest recommendation: teams starting a new Python project should use Basilisk and benefit from strict enforcement from day one — especially if they work across more than one editor. Teams migrating from Pyright on an existing well-typed codebase should evaluate as conformance approaches parity.
