---
layout: layouts/docs.njk
title: "Basilisk vs Pyright vs mypy vs ty vs Pyrefly"
description: "Detailed comparison of Python type checkers: Basilisk, Pyright, mypy, ty, and Pyrefly. Strictness, PEP conformance, and feature differences, with links to the measured benchmarks."
keywords: basilisk vs pyright, python type checker comparison, mypy vs basilisk, ty, pyrefly
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Comparison
  order: 8
---

# Comparing Type Checkers

The Python type checker landscape has changed significantly. Three new Rust-based tools launched in 2025. They differ in how faithfully they implement the typing spec, in whether they're a complete language server or only a checker — and in speed, which we [measure and publish](/docs/benchmarks/) rather than assert.

On the [official python/typing conformance suite]({{ conformanceOfficial.snapshot.source }}), **Basilisk is the only type checker with a perfect {{ conformanceOfficial.byId.basilisk.pct }}% score** ({{ conformanceOfficial.byId.basilisk.passLabel }}/{{ conformanceOfficial.byId.basilisk.total }}) — ahead of zuban ({{ conformanceOfficial.byId.zuban.pct }}%), Pyrefly ({{ conformanceOfficial.byId.pyrefly.pct }}%), Pyright ({{ conformanceOfficial.byId.pyright.pct }}%), ty ({{ conformanceOfficial.byId.ty.pct }}%), and mypy ({{ conformanceOfficial.byId.mypy.pct }}%), all graded on the same run.

## The fundamental question

Before comparing features and performance, there is one question that decides whether you can trust a checker's verdict at all:

**How much of the official typing specification does it actually implement?**

| Tool | PEP conformance (official suite¹) |
|---|---|
| **Basilisk** | **{{ conformanceOfficial.byId.basilisk.pct }}% ({{ conformanceOfficial.byId.basilisk.passLabel }}/{{ conformanceOfficial.byId.basilisk.total }})** |
| zuban | {{ conformanceOfficial.byId.zuban.pct }}% |
| Pyrefly | {{ conformanceOfficial.byId.pyrefly.pct }}% |
| Pyright | {{ conformanceOfficial.byId.pyright.pct }}% |
| pycroscope | {{ conformanceOfficial.byId.pycroscope.pct }}% |
| ty | {{ conformanceOfficial.byId.ty.pct }}% |
| mypy | {{ conformanceOfficial.byId.mypy.pct }}% |

Every score above is from **one identical run** of the official python/typing suite, which now grades Basilisk alongside every other checker — Basilisk tops it at a perfect {{ conformanceOfficial.byId.basilisk.pct }}%, the only tool on the board to do so.

A checker that doesn't implement a spec feature can't judge code that uses it — it either misses real errors or invents false ones. Basilisk's **default** rule set *is* the typing spec: it runs the core PEP conformance rules and nothing else, and passes every file in the suite at our pinned commit. Rule selection is entirely config-driven, so the default is exactly the core PEP set — never more.

Want checking stricter than the spec? Switch on the **opt-in Basilisk rules** in config. They're off by default and, by design, flag things the spec does *not* call errors (an unannotated parameter, say) — so turning them on will actually *break* strict spec conformance. That's the point: they're yours to enable when your team wants more than the spec, not something forced on every project.

---

## Full capability comparison

| Feature | Basilisk | Pyright | mypy | ty | Pyrefly |
|---|---|---|---|---|---|
| Enrichment fixes (auto-add types) | ✅ | ❌ | ❌ | ❌ | ❌ |
| Opt-in rules beyond the spec | ✅ config | `--strict` | `--strict` | ❌ | ❌ |
| PEP conformance¹ | **{{ conformanceOfficial.byId.basilisk.pct }}% — #1, only perfect score** | {{ conformanceOfficial.byId.pyright.pct }}% | {{ conformanceOfficial.byId.mypy.pct }}% | {{ conformanceOfficial.byId.ty.pct }}% | {{ conformanceOfficial.byId.pyrefly.pct }}% |
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

¹ Full-pass scores from one run of the [official python/typing conformance suite]({{ conformanceOfficial.snapshot.source }}), snapshot [python/typing@`{{ conformanceOfficial.snapshot.sha }}`]({{ conformanceOfficial.snapshot.prUrl }}) ({{ conformanceOfficial.snapshot.dateLabel }}): basilisk {{ conformanceOfficial.byId.basilisk.version }}, pyright {{ conformanceOfficial.byId.pyright.version }}, mypy {{ conformanceOfficial.byId.mypy.version }}, ty {{ conformanceOfficial.byId.ty.version }}, pyrefly {{ conformanceOfficial.byId.pyrefly.version }}, zuban {{ conformanceOfficial.byId.zuban.version }}. Basilisk is the only checker at a perfect {{ conformanceOfficial.byId.basilisk.pct }}%. These scores drift as the tools improve, so each links to its live results folder rather than a frozen figure.

---

## Pyright

**By Microsoft. TypeScript-based. {{ conformanceOfficial.byId.pyright.pct }}% PEP conformance on the official suite ([source]({{ conformanceOfficial.snapshot.source }})) — behind Basilisk's perfect {{ conformanceOfficial.byId.basilisk.pct }}%.**

Pyright was long the conformance front-runner and remains one of the most capable checkers. On the current official suite it scores {{ conformanceOfficial.byId.pyright.pct }}% — strong, but now behind Basilisk ({{ conformanceOfficial.byId.basilisk.pct }}%), zuban, and Pyrefly. It handles the vast majority of PEP typing features and has excellent performance for a TypeScript-based tool.

**What Pyright does well:**
- Strong PEP coverage ({{ conformanceOfficial.byId.pyright.pct }}% on the official conformance suite)
- Excellent documentation and error messages
- Deep VS Code integration via Pylance
- Fast enough for interactive use in most codebases
- Good inference for complex generics and protocols

**What Pyright doesn't do:**
- No integrated debugger or profiler — it checks types, but isn't a full dev environment
- Requires Node.js to run — adds a dependency to Python-only CI environments
- Pylance (the VS Code extension) is proprietary — its richest features don't leave VS Code
- No plugins — no way to add framework-specific type intelligence

**When Pyright makes sense:** Basilisk now exceeds Pyright's conformance ({{ conformanceOfficial.byId.basilisk.pct }}% vs {{ conformanceOfficial.byId.pyright.pct }}% on the official suite) while adding a full LSP, integrated debugger, and profiler. Pyright remains a strong, mature option if you're already invested in the Microsoft VS Code ecosystem and don't mind the Node.js dependency.

---

## mypy

**The original. Python/C-based. {{ conformanceOfficial.byId.mypy.pct }}% on the official suite ([source]({{ conformanceOfficial.snapshot.source }})) — versus Basilisk's perfect {{ conformanceOfficial.byId.basilisk.pct }}%.**

mypy defined what Python type checking looks like. Its `--strict` flag was the reference implementation for what "strict" means in Python typing for years.

**What mypy does well:**
- Established plugin ecosystem: Django, SQLAlchemy, Pydantic all have mypy plugins
- `--strict` flag is well-understood and documented
- Largest community and most StackOverflow answers
- Long history means most edge cases are handled

**What mypy doesn't do:**
- Slowest cold single-file check of the tools in [our measured benchmarks](/docs/benchmarks/) (its incremental cache narrows the gap on re-checks)
- Daemon mode (`dmypy`) is fragile under certain conditions
- Not a language server — no completions, hover, or go-to-definition
- Requires a Python runtime
- Plugin API is Python-only — no WASM portability

**When mypy makes sense:** Existing codebases with heavy investment in mypy plugins (Django, SQLAlchemy) may find migration effort significant until Basilisk's WASM plugin ecosystem reaches parity.

---

## ty (Astral)

**Built by the Ruff team. Rust + Salsa. {{ conformanceOfficial.byId.ty.pct }}% on the official suite ([source]({{ conformanceOfficial.snapshot.source }})) — still maturing, well behind Basilisk's perfect {{ conformanceOfficial.byId.basilisk.pct }}%.**

ty is the most interesting new entrant. It's built by the same team that created Ruff (now the de facto Python linter), uses a Salsa-based incremental architecture, is built in Rust like Basilisk, and has Astral's engineering velocity behind it.

**What ty does well:**
- Rust-based incremental architecture (Salsa)
- Built by a team with a track record of shipping
- MIT licensed, fully open source
- Sub-10ms incremental speed ([4.7ms on PyTorch](https://astral.sh/blog/ty), December 2025)

**What ty doesn't do (yet):**
- Scores {{ conformanceOfficial.byId.ty.pct }}% on the [official python/typing conformance suite]({{ conformanceOfficial.snapshot.source }}) — well behind Basilisk's perfect {{ conformanceOfficial.byId.basilisk.pct }}%; still maturing
- Gradual typing by default
- No integrated debugger or profiler

**When ty makes sense:** If you want to bet on Astral's velocity and can tolerate lower type coverage during the adoption period. ty may eventually become a major player; it's too early to depend on it for strict enforcement.

---

## Pyrefly (Meta)

**Production-tested at Instagram scale. Rust-based. {{ conformanceOfficial.byId.pyrefly.pct }}% PEP conformance on the official suite ([source]({{ conformanceOfficial.snapshot.source }})) — behind Basilisk's perfect {{ conformanceOfficial.byId.basilisk.pct }}%.**

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
1. The **only type checker with a perfect {{ conformanceOfficial.byId.basilisk.pct }}%** on the official python/typing suite — 100% PEP conformance out of the box, with **opt-in Basilisk rules** you switch on in config for checking stricter than the spec — they never run, and never touch the conformance score, unless you ask
2. Enrichment fixes — one-click code actions that add the missing types *for* you, instead of just reporting that they're missing
3. A complete, open-source LSP in every editor — completions, hover, go-to-definition, refactoring, debugging, and profiling, the same in VS Code, plus native Zed and Neovim extensions (Open VSX for Cursor, Windsurf, and others coming very soon; JetBrains planned) — not just inside one proprietary VS Code extension
4. Integrated debugger and profiler brokered through the language server
5. WASM plugin system (planned) — extensible without forking, secure by design

**Where Basilisk is not yet the best choice:**
- Maturity and edge-case breadth: Basilisk passes {{ conformance.scorePct }}% of the official suite ({{ conformance.pass }}/{{ conformance.total }}) at our [pinned commit](/docs/conformance/), counting errors *and* warnings — the strictest grading — with {{ conformance.fp }} false positives and {{ conformance.missed }} missed required errors. That measures spec conformance, not years of hardening: Pyright still handles more real-world edge cases beyond the suite, and Pylance is feature-complete today. Basilisk is in alpha.
- Plugin ecosystem: mypy's Django and SQLAlchemy plugins are mature. Basilisk's WASM plugins are planned.
- Maturity: Pylance is feature-complete today (though proprietary and VS Code only). Basilisk is in alpha.

The honest recommendation: teams starting a new Python project get full PEP-conformant checking from Basilisk on day one — with the option to switch on stricter-than-spec rules whenever they're ready — especially if they work across more than one editor. Teams migrating from Pyright on a large, well-typed codebase should still weigh Pyright's maturity and Pylance's feature completeness.
