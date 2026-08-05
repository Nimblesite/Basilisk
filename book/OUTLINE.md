# The Basilisk Book — structural outline

## Shape of the first edition

The first edition targets **about 29,000 words**, **about 105 print-equivalent
pages**, and **40 purposeful visuals**. EPUB pages reflow by device, so the word
and visual budgets are the controlling measurements; the page total is a
layout target, not a promise about screen count.

The structure borrows the best rhythm from *Instant Code: C#*—a quick success,
short demonstrations, comparisons, practice, summary, and official further
reading—without inheriting its roughly 105,000-word scale.

| Material | Words | Print-equivalent pages | Visuals |
|---|---:|---:|---:|
| Front matter | 1,200 | 5 | 1 |
| Part I — See the system | 6,200 | 20 | 8 |
| Part II — Think in types | 11,900 | 40 | 16 |
| Part III — Make it your workflow | 9,200 | 33 | 15 |
| Back matter | 1,200 | 7 | 0 |
| **Total** | **29,700** | **105** | **40** |

## Through-line: the Signal Box project

Readers progressively type and improve a small, dependency-light telemetry
application called **Signal Box**. It accepts JSON-like sensor readings,
normalizes them, produces alerts, stores them through an interface, and exposes
a small command-line report.

The project is deliberately ordinary. It creates natural opportunities for
unions, `None`, structured external data, dataclasses, enums, protocols,
generics, imports, a local stub, tests, refactoring, debugging, and a measurable
hot path. Every chapter ends at a runnable checkpoint. The book never relies on
toy fragments alone when the same lesson can be shown in the evolving project.

## Recurring chapter contract

Every chapter uses the same learning rhythm:

1. **The problem** — a concrete failure, question, or maintenance task.
2. **Basilisk in view** — a direct capture of the pinned release showing the
   relevant feedback. A mock, redraw, generated image, or UI-shaped diagram can
   never fill this slot.
3. **The idea** — one evidence-rich diagram and the smallest necessary theory.
4. **Before → diagnostic → after** — two to four short, executable examples.
5. **Guided checkpoint** — a change to Signal Box with explicit steps.
6. **Try it yourself** — one faded example and one independent variation.
7. **What changed** — a compact summary and bridge to the next chapter.
8. **Authoritative sources** — adjacent citations plus a short chapter list.

No chapter introduces more than four new conceptual families. Screenshots are
direct evidence of behaviour; diagrams explain relationships that screenshots
cannot and must never imitate product appearance.

## Front matter — How to use this book

**Target:** 1,200 words · 5 pages · 1 visual

- The promise and intended reader
- Prerequisites: ordinary Python, no prior static-typing expertise
- The maintained typing specification as the authority; Python-version
  boundaries appear only when a governing PEP or runtime behaviour requires one
- How to run, change, and re-check every example
- What a type checker can and cannot tell you
- Edition, Basilisk release, platform, and screenshot provenance
- Authority, corrections, accessibility, and free-publication notes
- Visual: the cover-to-cover journey map

## Part I — See the system

### Chapter 1 — Meet Basilisk

**Target:** 1,800 words · 6 pages · 2 visuals

**Reader outcome:** Explain where Basilisk sits between Python source, an
editor, the command line, and a running program.

- A Python program and a static question are different things
- The Basilisk system: checker, language server, editor clients, and developer
  tools
- The default Python-spec rules and opt-in Basilisk rules
- Feedback loops: edit, understand, change, verify
- The live website as the companion reference
- Checkpoint: open the finished Signal Box sample and identify each surface
- Visuals: system map; real full-product editor capture
- Source keys: `python-typing-spec`, `python-annotations`, `basilisk-home`,
  `basilisk-rules`, `lsp-spec`

### Chapter 2 — Your first ten minutes

**Target:** 2,200 words · 7 pages · 3 visuals

**Reader outcome:** Install the appropriate integration, check one file, and
reach a clean result without copying unexplained configuration.

- Choose an editor integration or the standalone CLI
- Confirm which Basilisk version is actually running
- Create a deliberately incompatible Signal Box call that violates a default
  Python-spec rule
- Run `basilisk check` and fix the first diagnostic
- Check one file, a directory, and the configured project roots
- Understand success and process exit status
- Checkpoint: commit-free local baseline with one clean module
- Visuals: installation decision map; terminal first check; editor first check
- Website destinations: installation, quick start, releases

### Chapter 3 — Read the evidence

**Target:** 2,200 words · 7 pages · 3 visuals

**Reader outcome:** Read severity, code, source span, help, note, and website
link; distinguish a symptom from the underlying type relationship.

- Anatomy of a Basilisk diagnostic
- Why stable rule identifiers matter
- Editor diagnostics and terminal diagnostics describe the same issue
- Hover as a view into inferred information
- Safe fixes, human decisions, and when not to accept a fix blindly
- Checkpoint: diagnose three different failures before changing the code
- Visuals: annotated diagnostic; hover screenshot; diagnosis feedback loop
- Website destinations: rule reference and per-diagnostic pages

## Part II — Think in types

### Chapter 4 — The everyday type vocabulary

**Target:** 2,400 words · 8 pages · 3 visuals

**Reader outcome:** Read and write the types used in ordinary Signal Box data
without confusing annotations with runtime conversion.

- Names, values, annotations, and runtime behavior
- Built-in scalar and collection types
- `T | None`, unions, and literal values
- Type aliases that clarify a domain without creating a distinct type
- Checkpoint: annotate raw readings and normalized readings
- Visuals: runtime/static split; union flow; alias equivalence diagram
- Source keys: `python-typing-docs`, `python-typing-spec-concepts`,
  `python-typing-spec-aliases`, `pep-585`, `pep-604`

### Chapter 5 — Compatibility is the question

**Target:** 2,400 words · 8 pages · 3 visuals

**Reader outcome:** Predict the common assignment, argument, and return errors
that Basilisk reports.

- The recurring question: can this value be used here?
- Assignment compatibility and subtyping in practical terms
- Function parameters and returns as a contract
- Mutable collections and why apparently similar types can differ
- Callables, callbacks, and variance only as far as the user needs it
- Checkpoint: make the alert formatter honest about its accepted inputs
- Visuals: compatibility gate; function contract; mutable collection trap
- Source keys: `python-typing-spec-concepts`,
  `python-typing-spec-callables`, `python-typing-spec-generics`

### Chapter 6 — Inference, narrowing, and all the paths

**Target:** 2,100 words · 7 pages · 3 visuals

**Reader outcome:** Distinguish normative narrowing guarantees from
checker-specific inference, then write and test every runtime path.

- Why the normative specification leaves general inference largely unspecified
- Declared boundaries and the specified `is None` union case
- Normative user-defined guards with `TypeGuard` and `TypeIs`
- Runtime branching where exact static narrowing is not specified
- Pattern matching and `assert_never` as guidance, not a portable guarantee
- Checkpoint: route all reading variants without a blind cast
- Visuals: normative boundary map; `TypeGuard` before/after; runtime case map
- Source keys: `python-typing-spec-narrowing`, `python-typing-docs`,
  `pep-647`, `python-match-statement`

### Chapter 7 — Reusable contracts and structured data

**Target:** 2,700 words · 9 pages · 4 visuals

**Reader outcome:** Choose among a dataclass, `TypedDict`, protocol, enum, and
generic abstraction based on the shape of the data and boundary.

- `TypedDict` for dictionary-shaped external data
- Dataclasses and named attributes for internal state
- Enums and literals for closed choices
- Protocols and structural subtyping for replaceable behavior
- Type parameters and one useful generic container
- Overloads: precise public behavior without duplicated implementation
- Checkpoint: separate raw input, domain model, and storage protocol
- Visuals: boundary transformation; nominal/structural comparison; generic
  payload; overload decision table
- Source keys: `python-typing-spec-typeddict`,
  `python-typing-spec-protocols`, `python-typing-spec-generics`,
  `python-dataclasses`, `python-enum`

### Chapter 8 — Imports, packages, and the world of stubs

**Target:** 2,300 words · 8 pages · 3 visuals

**Reader outcome:** Explain where imported type information comes from and
respond appropriately when a package has no usable typing information.

- Source modules, `.pyi` files, and what a stub promises
- Typeshed and the standard library
- Where standard-library types come from at run time: the verified latest
  commit, an exact `typeshed-commit` pin, a custom tree, or the offline
  bundled snapshot
- `py.typed` and typed distributions
- Import resolution from project overrides to installed packages
- Missing versus incomplete stubs
- Generate, inspect, and maintain a local stub
- Checkpoint: teach Basilisk about the simulated sensor vendor package
- Visuals: import-resolution stack; hover provenance; local-stub workflow
- Source keys: `python-typing-spec-distributing`, `pep-561`, `typeshed`,
  `packaging-pyproject`, `basilisk-configuration`

## Part III — Make it your workflow

### Chapter 9 — Configure the project, not a mood

**Target:** 2,200 words · 8 pages · 3 visuals

**Reader outcome:** Express a reviewable rule policy in `pyproject.toml`, then
use the real configuration editor to preview and apply a bounded change.

- Python typing semantics versus opt-in project policy
- The active root configuration source
- Live rule catalog, tags, search, and rule details
- Inherited and Native intentions versus four persisted severities
- Exact preview/apply flow and stale-revision protection
- Project severity and one bounded test-path override
- Presets as explicit recipes rather than policy modes
- Checkpoint: required annotations in Signal Box source and a warning in tests
- Visuals: real configuration-editor capture; preview transaction diagram;
  real preview capture
- Website destinations: configuration and rules

### Chapter 10 — Adopt a codebase without hiding it

**Target:** 2,200 words · 8 pages · 3 visuals

**Reader outcome:** Move an existing codebase toward the chosen policy while
keeping unfinished work visible and measurable.

- Inventory before fixing
- Fix the high-confidence transformations first
- `fix`, `adopt`, status, and `unadopt` as an intentional workflow
- Errors, warnings, and gradual change
- Work from boundaries inward
- Review generated annotations instead of worshipping them
- Checkpoint: migrate the deliberately untyped Signal Box legacy module
- Visuals: adoption funnel; CLI fix; file status before/after
- Website destination: migration guide

### Chapter 11 — Let the editor carry context

**Target:** 2,300 words · 8 pages · 4 visuals

**Reader outcome:** Use language-server features as one connected workflow,
not as a list of unrelated commands.

- Hover, completion, signature help, and inlay hints
- Definition, references, symbols, and type hierarchy
- Rename and refactoring with a preview mindset
- Quick fixes, source actions, formatting, and import organization
- Editor notes for VS Code-family editors, Neovim, and Zed
- Checkpoint: extract, rename, navigate, and format across Signal Box modules
- Visuals: LSP request loop; hover; rename preview; quick-fix preview
- Source keys: `lsp-spec`, `basilisk-refactoring`, `basilisk-installation`

### Chapter 12 — Run, investigate, and ship

**Target:** 2,500 words · 9 pages · 5 visuals

**Reader outcome:** Connect static feedback to tests and runtime evidence, then
make the same checks repeatable in CI.

- Type checking is one line of evidence, not execution
- Discover and run tests from the editor
- Start a debug session, stop at a breakpoint, and inspect state
- Find a CPU hot path and interpret a flame graph
- Stable CLI output, JSON output, caching, and CI exit behavior
- Pin the documented Basilisk version and link to release notes
- Final checkpoint: check, test, debug, profile, and CI the complete project
- Visuals: evidence loop; test explorer; debugger; profiler; CI terminal
- Source keys: `python-pdb`, `dap-spec`,
  `basilisk-debugging`, `basilisk-profiler`, `basilisk-releases`

## Back matter

**Target:** 1,200 words · 7 pages

- Appendix A — Command and configuration quick reference, generated from the
  documented release rather than typed by hand
- Appendix B — Python typing vocabulary and notation
- Appendix C — Source map: typing spec topics, Python docs, CPython, typeshed,
  and Basilisk website destinations
- Appendix D — Screenshot environment and reproducibility record
- Where to go next: live docs, rules, releases, GitHub, and corrections

## Explicitly out of scope for the first edition

- Basilisk's internal Rust crate architecture
- An exhaustive restatement of every typing-spec rule
- A complete Python language tutorial
- Competitor feature or performance comparisons
- Unshipped commands or roadmap promises presented as current behavior
- Generated, mocked, redrawn, reconstructed, or hand-composed UI screenshots,
  including UI imitations relabelled as diagrams
