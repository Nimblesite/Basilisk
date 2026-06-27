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
  order: 8
---

# Comparing Type Checkers

The Python type checker landscape has changed significantly. Three new Rust-based tools launched in 2025. They differ less in raw speed than in how faithfully they implement the typing spec — and in whether they're a complete language server or only a checker.

## The fundamental question

Before comparing features and performance, there is one question that decides whether you can trust a checker's verdict at all:

**How much of the official typing specification does it actually implement?**

| Tool | PEP conformance (official suite¹) |
|---|---|
| Basilisk | {{ conformance.scorePct }}% ({{ conformance.pass }}/{{ conformance.total }}) |
| Pyright | ~99% |
| Pyrefly | ~86% |
| mypy | ~58% full-pass |
| ty | early alpha |

A checker that doesn't implement a spec feature can't judge code that uses it — it either misses real errors or invents false ones. Basilisk's **default** rule set *is* the typing spec: it runs the core PEP conformance rules and nothing else, and passes every file in the suite at our pinned commit. Rule selection is entirely config-driven, so the default is exactly the core PEP set — never more.

Want checking stricter than the spec? Switch on the **opt-in Basilisk rules** in config. They're off by default and, by design, flag things the spec does *not* call errors (an unannotated parameter, say) — so turning them on will actually *break* strict spec conformance. That's the point: they're yours to enable when your team wants more than the spec, not something forced on every project.

---

## Full capability comparison

| Feature | Basilisk | Pyright | mypy | ty | Pyrefly |
|---|---|---|---|---|---|
| Enrichment fixes (auto-add types) | ✅ | ❌ | ❌ | ❌ | ❌ |
| Opt-in rules beyond the spec | ✅ config | `--strict` | `--strict` | ❌ | ❌ |
| PEP conformance¹ | {{ conformance.scorePct }}% current (→100% target) | ~99% | ~58% | early alpha | ~86% |
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
- No integrated debugger or profiler — it checks types, but isn't a full dev environment
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
- Not a language server — no completions, hover, or go-to-definition
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
- No integrated debugger or profiler
- No plugin system
- Meta-driven roadmap — external contributions have less influence

**When Pyrefly makes sense:** Extremely large codebases (500K+ LOC) where throughput matters more than strict enforcement, particularly if the team has Meta-adjacent tooling.

---

## Basilisk's position

Basilisk is not a faster version of an existing tool. It occupies a different position:

**Unique to Basilisk:**
1. 100% PEP conformance out of the box (at our pinned suite commit), with **opt-in Basilisk rules** you switch on in config for checking stricter than the spec — they never run, and never touch the conformance score, unless you ask
2. Enrichment fixes — one-click code actions that add the missing types *for* you, instead of just reporting that they're missing
3. A complete, open-source LSP in every editor — completions, hover, go-to-definition, refactoring, debugging, and profiling, the same in VS Code, plus native Zed and Neovim extensions (Open VSX for Cursor, Windsurf, and others coming very soon; JetBrains planned) — not just inside one proprietary VS Code extension
4. Integrated debugger and profiler brokered through the language server
5. WASM plugin system (planned) — extensible without forking, secure by design

**Where Basilisk is not yet the best choice:**
- Maturity and edge-case breadth: Basilisk passes {{ conformance.scorePct }}% of the official suite ({{ conformance.pass }}/{{ conformance.total }}) at our [pinned commit](/docs/conformance/), counting errors *and* warnings — the strictest grading — with {{ conformance.fp }} false positives and {{ conformance.missed }} missed required errors. That measures spec conformance, not years of hardening: Pyright still handles more real-world edge cases beyond the suite, and Pylance is feature-complete today. Basilisk is in alpha.
- Plugin ecosystem: mypy's Django and SQLAlchemy plugins are mature. Basilisk's WASM plugins are planned.
- Maturity: Pylance is feature-complete today (though proprietary and VS Code only). Basilisk is in alpha.

The honest recommendation: teams starting a new Python project get full PEP-conformant checking from Basilisk on day one — with the option to switch on stricter-than-spec rules whenever they're ready — especially if they work across more than one editor. Teams migrating from Pyright on a large, well-typed codebase should still weigh Pyright's maturity and Pylance's feature completeness.
