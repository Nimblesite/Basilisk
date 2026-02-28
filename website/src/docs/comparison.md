---
layout: layouts/docs.njk
title: Comparing Type Checkers
description: How Basilisk compares to Pyright, mypy, ty, and Pyrefly across strictness, performance, and features.
keywords: basilisk vs pyright, python type checker comparison, mypy vs basilisk, ty, pyrefly
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
| PEP conformance | 100% target | ~95% | ~85% | ~15% | ~58% |
| Implementation | Rust | TypeScript | Python/C | Rust | Rust |
| Runtime required | None | Node.js | Python | None | None |
| Incremental speed | <10ms | ~50ms | ~100ms | <10ms | <10ms |
| Ownership analysis | ✅ | ❌ | ❌ | ❌ | ❌ |
| Immutability enforcement | ✅ | ❌ | ❌ | ❌ | ❌ |
| Coercion detection | ✅ | ❌ | ❌ | ❌ | ❌ |
| Plugin system | WASM | None | Python hooks | Planned | None |
| VS Code extension | Open source | Proprietary (Pylance) | None | Open source | Open source |
| Mojo compatibility | ✅ | ❌ | ❌ | ❌ | ❌ |
| SARIF output | ✅ (Phase 3) | ✅ | ❌ | ❌ | ✅ |
| License | MIT / Apache-2.0 | MIT | MIT | MIT | MIT |

---

## Pyright

**By Microsoft. TypeScript-based. ~95% PEP conformance.**

Pyright is the most conformant Python type checker available today. It correctly handles the vast majority of PEP typing features and has excellent performance for a TypeScript-based tool.

**What Pyright does well:**
- Highest PEP coverage of any shipping tool (~95%)
- Excellent documentation and error messages
- Deep VS Code integration via Pylance
- Fast enough for interactive use in most codebases
- Good inference for complex generics and protocols

**What Pyright doesn't do:**
- Strict by default — four modes: `off`, `basic`, `standard`, `strict`
- Requires Node.js to run — adds a dependency to Python-only CI environments
- Pylance (the VS Code extension) is proprietary — not all features are available outside VS Code
- No ownership or immutability analysis
- No plugins — no way to add framework-specific type intelligence
- No coercion detection

**When Pyright makes sense:** If you're already invested in the Microsoft VS Code ecosystem and don't mind the Node.js dependency, Pyright's current PEP conformance makes it the strongest choice for pure type checking today. Basilisk targets exceeding its conformance in Phase 3.

---

## mypy

**The original. Python/C-based. ~85% PEP conformance.**

mypy defined what Python type checking looks like. Its `--strict` flag was the reference implementation for what "strict" means in Python typing for years.

**What mypy does well:**
- Established plugin ecosystem: Django, SQLAlchemy, Pydantic all have mypy plugins
- `--strict` flag is well-understood and documented
- Largest community and most StackOverflow answers
- Long history means most edge cases are handled

**What mypy doesn't do:**
- 10–100x slower than Rust-based tools on large codebases
- Daemon mode (`dmypy`) is fragile under certain conditions
- Not strict by default
- Requires a Python runtime
- No ownership analysis, no coercion detection
- Plugin API is Python-only — no WASM portability

**When mypy makes sense:** Existing codebases with heavy investment in mypy plugins (Django, SQLAlchemy) may find migration effort significant until Basilisk's WASM plugin ecosystem reaches parity.

---

## ty (Astral)

**Built by the Ruff team. Rust + Salsa. ~15% PEP conformance.**

ty is the most interesting new entrant. It's built by the same team that created Ruff (now the de facto Python linter), uses the same Salsa-based incremental architecture as Basilisk, and has Astral's engineering velocity behind it.

**What ty does well:**
- Same architectural foundation as Basilisk (Salsa + Rust)
- Built by a team with a track record of shipping
- MIT licensed, fully open source
- Sub-10ms incremental speed when it reaches feature parity

**What ty doesn't do (yet):**
- Only ~15% PEP conformance as of late 2025 — early beta
- Gradual typing by default
- No ownership analysis
- Years away from Pyright's coverage level

**When ty makes sense:** If you want to bet on Astral's velocity and can tolerate lower type coverage during the adoption period. ty may eventually become a major player; it's too early to depend on it for strict enforcement.

---

## Pyrefly (Meta)

**Production-tested at Instagram scale. Rust-based. ~58% PEP conformance.**

Pyrefly was built by Meta to handle their Python codebase — one of the largest in the world. It emphasizes throughput (1.8M LOC/sec) over coverage.

**What Pyrefly does well:**
- Battle-tested on millions of lines of production Python
- High throughput suitable for monorepo-scale codebases
- Rust-based, no runtime dependency
- Good documentation

**What Pyrefly doesn't do:**
- ~58% PEP conformance — below Pyright and below Basilisk's target
- Strict by default — not available
- No ownership or immutability analysis
- No plugin system
- Meta-driven roadmap — external contributions have less influence

**When Pyrefly makes sense:** Extremely large codebases (500K+ LOC) where throughput matters more than coverage completeness, particularly if the team has Meta-adjacent tooling.

---

## Basilisk's position

Basilisk is not a faster version of an existing tool. It occupies a different position:

**Unique to Basilisk:**
1. Strict by default — the only tool where you cannot accidentally run in permissive mode
2. Ownership analysis — `Borrowed`, `InOut`, `Owned` semantics, statically verified
3. Immutability enforcement — parameters are read-only unless declared otherwise
4. Coercion detection — implicit `int`→`float`, `bool`→`int`, `bytes`→`str` are type errors
5. WASM plugin system — extensible without forking, secure by design
6. Mojo compatibility — code passing Basilisk is structurally ready for Mojo

**Where Basilisk is not yet the best choice:**
- PEP conformance: Phase 1 implements E0001–E0025. Pyright covers more edge cases today. Basilisk's target is 100%; it's not there yet.
- Plugin ecosystem: mypy's Django and SQLAlchemy plugins are mature. Basilisk's WASM plugins are Phase 5.
- VS Code extension: The Basilisk extension is Phase 2. Pylance is feature-complete today (though proprietary).

The honest recommendation: teams starting a new Python project should use Basilisk and benefit from strict enforcement from day one. Teams migrating from Pyright on an existing well-typed codebase should evaluate in Phase 3 when coverage reaches parity.
