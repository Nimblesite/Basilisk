---
layout: layouts/docs.njk
title: "Best Python Type Checker? Basilisk vs Pyright & mypy"
description: "Which Python type checker is best? Compare Basilisk, Pyright, mypy, ty, and Pyrefly on official PEP conformance, strictness, editor support, and benchmarks."
keywords: best python type checker, basilisk vs pyright, python type checker comparison, mypy vs basilisk, ty, pyrefly
date: 2026-02-28
dateModified: 2026-08-06
author: The Basilisk Project
eleventyNavigation:
  key: Comparison
  order: 8
---

# What Is the Best Python Type Checker?

There is no universal **best Python type checker** for every codebase. The right choice depends on what you value most: typing-spec conformance, mature framework plugins, editor integration, performance, or a complete language-server workflow. This comparison makes those tradeoffs explicit and links every changing score to its source.

The Python type checker landscape has changed significantly. The tools differ in how faithfully they implement the typing spec, in whether they're a complete language server or only a checker, and in speed. Basilisk's previously published performance measurements are currently [withdrawn pending review](/docs/benchmarks/).

<p class="bench-caveat"><strong>Conformance correction:</strong> Basilisk's former result is withdrawn and its current percentage is temporarily unknown. Basilisk has been removed from the <a href="https://github.com/python/typing/blob/main/conformance/results/results.html">official results table</a> at our request while affected logic is rebuilt and stress-tested beyond the exact suite fixtures. Do not use the old score or leaderboard position to compare these tools.</p>

## The fundamental question

Before comparing features and performance, there is one question that decides whether you can trust a checker's verdict at all:

**How much of the official typing specification does it actually implement, beyond the exact tests used to measure it?**

The [official results table](https://github.com/python/typing/blob/main/conformance/results/results.html) remains the source for checkers currently listed there. Basilisk is not currently listed. Its old figure failed robustness checks against semantics-preserving test mutations, so the honest answer for Basilisk is temporarily **unknown** while the affected implementation is replaced. See the [conformance correction](/docs/conformance/).

Want checking stricter than the spec? Switch on the **opt-in Basilisk rules** in config. They're off by default and, by design, flag things the spec does *not* call errors (an unannotated parameter, say), so turning them on will actually *break* strict spec conformance. That's the point: they're yours to enable when your team wants more than the spec, not something forced on every project.

---

## Full capability comparison

Every tick, cross, and label below links to the primary source (official docs, repo, or LICENSE) that backs it. Where a tool does more than one row can hold, the footnote says so.

| Feature | Basilisk | Pyright | mypy | ty | Pyrefly |
|---|---|---|---|---|---|
| Annotation quick-fix (inserts placeholder) | ✅ `: Any` / `-> None` ² | ❌ ³ | ❌ ⁴ | double-click inlay hint ⁵ | ❌ (code action) |
| Auto-insert *inferred* types | ❌ | ❌ ³ | ❌ ⁴ | ❌ | ✅ CLI `pyrefly infer` ⁶ |
| Opt-in rules beyond the spec | ✅ config | strict mode ⁷ | `--strict` ⁴ | severities only ⁸ | ✅ `strict` preset ⁹ |
| PEP conformance¹ | **Temporarily unknown; old result withdrawn** | See live results | See live results | See live results | See live results |
| Implementation | Rust | TypeScript ³ | Python/C ⁴ | Rust ¹⁰ | Rust ¹¹ |
| Runtime required | None | Node.js ³ | Python ⁴ | None ¹⁰ | None ¹¹ |
| Completions, hover, goto | ✅ | ✅ ¹² | ❌ ⁴ | ✅ ¹³ | ✅ ¹⁴ |
| Integrated debugger | ✅ | ❌ ³ | ❌ ⁴ | ❌ ¹³ | ❌ ¹⁴ |
| Integrated profiler | ✅ | ❌ ³ | ❌ ⁴ | ❌ ¹³ | ❌ ¹⁴ |
| Editor extensions | VS Code, Zed, Neovim (Open VSX for Cursor/Windsurf soon) | VS Code (OSS + proprietary Pylance) ¹⁵ | none official ¹⁶ | VS Code, PyCharm, Neovim, Zed ¹⁷ | VS Code + many via LSP ¹⁴ |
| Plugin system | WASM (planned) | None ³ | Python hooks ¹⁸ | None ¹⁹ | None ⁹ |
| License | MIT | MIT ²⁰ (Pylance proprietary ¹⁵) | MIT ²¹ | MIT ²² | MIT ²³ |

<a name="footnotes"></a>

**Sources:**

¹ See the [live official python/typing results](https://github.com/python/typing/blob/main/conformance/results/results.html) for checkers currently listed there. Basilisk requested removal after retracting its former result; its current percentage will be published only after the clean implementation passes robustness and mutation verification.

² Basilisk's quick-fix inserts a **placeholder** annotation (`: Any` on parameters and attributes, `-> None` on returns; empty-collection variables get `list[Any]` / `dict[str, Any]`) for you to replace with the real type. It does not infer types. See [Missing annotation rules](/docs/rules/missing-annotations/).

³ [Pyright docs](https://microsoft.github.io/pyright/#/features) and [source](https://github.com/microsoft/pyright): Pyright's only source quick-fix is "Create Type Stub"; it is written in TypeScript, requires [Node.js](https://microsoft.github.io/pyright/#/installation), and has [no plugin mechanism](https://microsoft.github.io/pyright/#/configuration). It is a type checker only, with no debugger or profiler.

⁴ [mypy docs](https://mypy.readthedocs.io/en/stable/): mypy is [written in Python and compiled with mypyc](https://mypyc.readthedocs.io/en/latest/introduction.html), requires a Python runtime, has a [`--strict`](https://mypy.readthedocs.io/en/stable/command_line.html) flag, and is not a language server (the [`dmypy` daemon](https://mypy.readthedocs.io/en/stable/mypy_daemon.html) accelerates checking, not completions/hover/goto). It emits draft signatures via `dmypy suggest`, but a separate tool (PyAnnotate) writes them, so mypy itself does not insert annotations.

⁵ [ty language server docs](https://docs.astral.sh/ty/features/language-server/): ty has no automated add-annotation code action, but its inlay hints "can be double-clicked to insert the type annotations into your source code."

⁶ [Pyrefly `autotype` docs](https://pyrefly.org/en/docs/autotype/): `pyrefly infer` writes **inferred** parameter, return, and container annotations directly into source (CLI, in active development). This is more than Basilisk's placeholder fix, so the row is credited to Pyrefly, not to us.

⁷ [Pyright configuration](https://microsoft.github.io/pyright/#/configuration): strict checking is enabled via `typeCheckingMode: "strict"` (config setting or `# pyright: strict`), not a `--strict` CLI flag.

⁸ [ty rules](https://docs.astral.sh/ty/rules/): ty lets you change rule **severities** (`--error all` escalates existing spec rules) but documents no beyond-spec strict preset.

⁹ [Pyrefly configuration](https://pyrefly.org/en/docs/configuration/): the `strict` preset enables extra checks (`implicit-any`, `missing-override-decorator`, and more) and any error code can be opted in individually. Pyrefly documents no plugin system.

¹⁰ [ty repo](https://github.com/astral-sh/ty) and [installation](https://docs.astral.sh/ty/installation/): written in Rust (Salsa), ships as a standalone binary with no Node.js/Python runtime.

¹¹ [Pyrefly repo](https://github.com/facebook/pyrefly) and [installation](https://pyrefly.org/en/docs/installation/): written in Rust, ships as a standalone binary with no runtime dependency.

¹² [Pyright features](https://microsoft.github.io/pyright/#/features): the open-source (MIT) pyright language server provides completions, hover, and go-to-definition; the proprietary [Pylance](https://github.com/microsoft/pylance-release/blob/main/FAQ.md) adds semantic highlighting, refactorings, and IntelliCode on top.

¹³ [ty language server](https://docs.astral.sh/ty/features/language-server/): implements completions, hover, goto, references, rename, signature help, code actions, and more (formatting is delegated to Ruff). No debugger or profiler.

¹⁴ [Pyrefly IDE docs](https://pyrefly.org/en/docs/IDE/): a full-featured language server (hover, completion, definition, references, rename, code actions, call hierarchy, and more) with a first-party VS Code/Open VSX extension and documented setup for Neovim, Vim, Emacs, JetBrains, Zed, Helix, Sublime, and Jupyter via LSP. No debugger or profiler.

¹⁵ [Pyright README](https://github.com/microsoft/pyright): ships an open-source (MIT) VS Code extension; Microsoft's richer default experience, [Pylance](https://marketplace.visualstudio.com/items?itemName=ms-python.vscode-pylance), is [proprietary](https://github.com/microsoft/pylance-release/blob/main/FAQ.md).

¹⁶ The mypy project ships no first-party editor extension; a Microsoft-maintained third-party one, [`ms-python.mypy-type-checker`](https://marketplace.visualstudio.com/items?itemName=ms-python.mypy-type-checker), provides diagnostics only.

¹⁷ [ty editors docs](https://docs.astral.sh/ty/editors/): official/first-class support for VS Code (Astral-maintained extension), PyCharm (2025.3+), Neovim, and Zed, plus any LSP editor via `ty server`.

¹⁸ [Extending mypy](https://mypy.readthedocs.io/en/stable/extending_mypy.html): mypy has a Python plugin API (subclass `mypy.plugin.Plugin`) used by the Django, SQLAlchemy, and Pydantic plugins.

¹⁹ ty's [docs](https://docs.astral.sh/ty/) and [launch announcement](https://astral.sh/blog/ty) document no plugin system and none announced as planned.

²⁰ [Pyright LICENSE.txt](https://github.com/microsoft/pyright/blob/main/LICENSE.txt), MIT.

²¹ [mypy LICENSE](https://github.com/python/mypy/blob/master/LICENSE), MIT.

²² [ty LICENSE](https://github.com/astral-sh/ty/blob/main/LICENSE), MIT.

²³ [Pyrefly LICENSE](https://github.com/facebook/pyrefly/blob/main/LICENSE), MIT.

---

## Pyright

**By Microsoft. TypeScript-based. See its current entry in the [official conformance results](https://github.com/python/typing/blob/main/conformance/results/results.html).**

Pyright was long the conformance front-runner and remains one of the most capable checkers. It handles a broad range of PEP typing features and has a mature editor ecosystem.

**What Pyright does well:**
- Strong PEP coverage; see the live official conformance results
- Excellent documentation and error messages
- Deep VS Code integration via Pylance
- Fast enough for interactive use in most codebases
- Good inference for complex generics and protocols

**What Pyright doesn't do:**
- No integrated debugger or profiler: it checks types, but isn't a full dev environment
- Requires Node.js to run, which adds a dependency to Python-only CI environments
- Pylance (the VS Code extension) is proprietary: its richest features don't leave VS Code
- No plugins, so there is no way to add framework-specific type intelligence

**When Pyright makes sense:** Pyright remains a strong, mature option if you're already invested in the Microsoft VS Code ecosystem and don't mind the Node.js dependency.

---

## mypy

**The original. Python/C-based. See its current entry in the [official conformance results](https://github.com/python/typing/blob/main/conformance/results/results.html).**

mypy defined what Python type checking looks like. Its `--strict` flag was the reference implementation for what "strict" means in Python typing for years.

**What mypy does well:**
- Established plugin ecosystem: Django, SQLAlchemy, Pydantic all have mypy plugins
- `--strict` flag is well-understood and documented
- Largest community and most StackOverflow answers
- Long history means most edge cases are handled

**What mypy doesn't do:**
- Requires a Python runtime for checking
- Daemon mode (`dmypy`) is fragile under certain conditions
- Not a language server, no completions, hover, or go-to-definition
- Requires a Python runtime
- Plugin API is Python-only, with no WASM portability

**When mypy makes sense:** Existing codebases with heavy investment in mypy plugins (Django, SQLAlchemy) may find migration effort significant until Basilisk's WASM plugin ecosystem reaches parity.

---

## ty (Astral)

**Built by the Ruff team. Rust + Salsa. See its current entry in the [official conformance results](https://github.com/python/typing/blob/main/conformance/results/results.html).**

ty is the most interesting new entrant. It's built by the same team that created Ruff (now the de facto Python linter), uses a Salsa-based incremental architecture, is built in Rust like Basilisk, and has Astral's engineering velocity behind it.

**What ty does well:**
- Rust-based incremental architecture (Salsa)
- Built by a team with a track record of shipping
- MIT licensed, fully open source
- Sub-10ms incremental speed ([4.7ms on PyTorch](https://astral.sh/blog/ty), December 2025)

**What ty doesn't do (yet):**
- Its typing implementation is still maturing
- Gradual typing by default
- No integrated debugger or profiler

**When ty makes sense:** If you value Astral's tooling ecosystem and are comfortable adopting a rapidly evolving checker.

---

## Pyrefly (Meta)

**Production-tested at Instagram scale. Rust-based. See its current entry in the [official conformance results](https://github.com/python/typing/blob/main/conformance/results/results.html).**

Pyrefly was built by Meta to handle their Python codebase, one of the largest in the world. It emphasizes throughput ([1.85M LOC/sec on 166-core Meta infrastructure](https://pyrefly.org/)) over strict enforcement.

**What Pyrefly does well:**
- Battle-tested on millions of lines of production Python
- High throughput suitable for monorepo-scale codebases
- Rust-based, no runtime dependency
- Good documentation

**What Pyrefly doesn't do:**
- No integrated debugger or profiler
- No plugin system
- Meta-driven roadmap, so external contributions have less influence

**When Pyrefly makes sense:** Extremely large codebases (500K+ LOC) where throughput matters more than strict enforcement, particularly if the team has Meta-adjacent tooling.

---

## Basilisk's position

Basilisk is not a faster version of an existing tool. It occupies a different position:

**Basilisk combines:**
1. Typing-spec rules enabled by default, plus **opt-in Basilisk rules** for checking stricter than the spec. The conformance implementation is currently being rebuilt and its percentage is temporarily unknown.
2. Annotation quick-fixes, one-click code actions that insert a placeholder annotation (`: Any`, `-> None`) on unannotated code, so you can fill in the real type instead of finding the spot by hand
3. A complete, open-source LSP in every editor, completions, hover, go-to-definition, refactoring, debugging, and profiling, the same in VS Code, plus native Zed and Neovim extensions (Open VSX for Cursor, Windsurf, and others coming very soon; JetBrains planned), not just inside one proprietary VS Code extension
4. Integrated debugger and profiler brokered through the language server
5. WASM plugin system (planned), extensible without forking, secure by design

**Where Basilisk is still growing:**
- Basilisk is under active development. Its former conformance result is withdrawn; affected logic is being reimplemented from scratch and the [current percentage is temporarily unknown](/docs/conformance/).
- Plugin ecosystem: mypy's Django and SQLAlchemy plugins are mature. Basilisk's WASM plugins are planned.

The recommendation: evaluate Basilisk for its integrated open-source editor workflow and test it against your own code. Do not choose it on the basis of the withdrawn conformance or benchmark figures. A new conformance result will be published when the clean implementation and robustness review are complete.
