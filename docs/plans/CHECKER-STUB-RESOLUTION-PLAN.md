# Stub Resolution & Custom Typeshed — Implementation Plan {#STUBRESPLAN}

> **Spec**: [CHECKER-STUB-RESOLUTION-SPEC.md §STUBRES](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES) — read before touching any code.
> **Scope**: everything under import-resolution / stub discovery / typeshed — the six-step
> [typing-spec import-resolution ordering](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)
> and its Basilisk mechanisms. Carved out of [LSP-PLAN.md](LSP-PLAN.md) so the remaining work is obvious in one place.
> **The actionable TODO list is at the very bottom: [§STUBRESPLAN-TODO](#STUBRESPLAN-TODO).**

---

## The request — issue #271 {#STUBRESPLAN-REQUEST}

[Issue #271, "Provide ability to override stdlib stubs / custom typeshed"](https://github.com/Nimblesite/Basilisk/issues/271),
from **Jos Verlinde**, maintainer of [`micropython-stubs`](https://github.com/Josverl/micropython-stubs). Verbatim, the crux:

> I notice that for basilisk it does not appear to be possible to specify a different set of stubs for the stdlib
> modules, which for MicroPython is essential as there are stdlib modules with additional methods, different
> signatures. for this there is a `micropython-stdlib-stubs` package - that is intended to replace the
> "typeshedfallback" package that most typecheckers include in their distributions, as I understand basilisk does as well.

What "fully satisfying #271" requires — each is tracked in the [TODO](#STUBRESPLAN-TODO):

1. **A custom-typeshed option** so MicroPython's stdlib stubs replace the vendored ones (typing-spec step 3). This is the feature.
2. **Correct handling of a _partial_ custom typeshed.** `micropython-stdlib-stubs` is *"a limited size copy of typeshed's
   stdlib directory"* ([PyPI](https://pypi.org/project/micropython-stdlib-stubs/)) with MicroPython-specific edits (e.g. `collections`) — **not** a full stdlib. Jos's explicit warning: *"Note that pyright and ty needed some changes to
   resolution order."* Getting the fall-through / canonicality right is the hard part, not the config key — see
   [§STUBRESPLAN-RESOLUTION-ORDER](#STUBRESPLAN-RESOLUTION-ORDER).
3. **Doc clarity** — he hit contradictory docs and no config mention. Those contradictions are already fixed
   (spec/website/README/plan now agree); this plan is the single home for the remaining work.
4. **A validation path with the reporter.** He offered: *"Happy to help in testing."* Ship behind a checklist that
   ends in a real `micropython-stdlib-stubs` smoke test he can confirm.

---

## Status {#STUBRESPLAN-STATUS}

The resolution **order** is implemented (`crates/basilisk-checker/src/imports/resolve.rs`,
[§STUBRES-PEP561](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PEP561)), including the step-3
custom-typeshed override (`typeshed-path`). The remaining #271 work is validation and release hardening:
MicroPython smoke coverage with the reporter and the full verification gate.

| Spec step | Mechanism | Config key | State |
|---|---|---|---|
| 1 — manual stubs at head of path | `.pyi` dirs prepended in `resolve.rs` | `stub-paths` / `stubPaths` | **DONE** |
| 2 — user code | workspace roots / `include` | roots, `include` | **DONE** |
| 3 — stdlib typeshed | bundled name-set recognition; **custom override** | `typeshed-path` / `typeshedPath` | override **IMPLEMENTED**, validation pending ([#271](https://github.com/Nimblesite/Basilisk/issues/271)) |
| 4 — stub-only packages | `foopkg-stubs` discovery in site-packages | (auto) | **DONE** |
| 5 — `py.typed` packages | `py.typed` marker detection | (auto) | **DONE** |
| 6 — vendored third-party stubs | intentionally empty (resolution vendors none) | — | **N/A by design** |

---

## Type model {#STUBRESPLAN-TYPES}

The normative stub-resolution type model now lives in the spec:
[§STUBRES-TYPE-MODEL](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPE-MODEL).
Keep this anchor as the plan-side pointer for implementation notes and checklist references.

---

## Done {#STUBRESPLAN-DONE}

Foundation that already ships (migrated here from LSP-PLAN tasks 7.5 / 7.6):

- [x] **`.pyi` parser** — signatures, class defs, variable annotations, `@overload`; bodies ignored (`pyi_parser.rs`, [§STUBRES-PYI](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PYI)).
- [x] **Resolution order (steps 1, 2, 4, 5)** — `resolve_module` walks stub-paths → user source → `foopkg-stubs` → `py.typed`/plain packages (`resolve.rs`, [§STUBRES-PEP561](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PEP561)).
- [x] **`stub-paths` (step 1)** — extra `.pyi` directories prepended at the head of the search path; may shadow any later module, stdlib or third-party. Parsed in both `basilisk-config` and `basilisk-lsp`, relative to project root.
- [x] **typeshed bundling (step 3 baseline)** — `build.rs` produces a `phf` stdlib module set for O(1) recognition ([§STUBRES-TYPESHED](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED)).
- [x] **`py.typed` detection** — inline-typed package marker per PEP 561.
- [x] **Provenance tiers + diagnostics** — Tier 1/2/3 provenance drives hover text and the `imports_unresolved` cascade suppression ([§STUBRES-PROVENANCE](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PROVENANCE)).
- [x] **Create-local stub** — `basilisk.stubs.createLocal` scaffolds a strict `.basilisk/stubs/{module}.pyi` ([§STUBRES-CREATE-LOCAL](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CREATE-LOCAL)).
- [x] **Add-member quick fix** — `basilisk.stubs.addMember` appends an undeclared member to a local stub ([§STUBRES-ADD-MEMBER](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-ADD-MEMBER)).

---

## Prior art — do it the way the popular checkers do {#STUBRESPLAN-PRIOR-ART}

The [typing spec's import-resolution ordering](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)
is the normative source. Its two user-facing knobs, quoted **verbatim**:

> 1. Stubs or Python source manually put in the beginning of the path. Type
>    checkers SHOULD provide this to allow the user complete control of which
>    stubs to use, and to patch broken stubs or inline types from packages. In
>    mypy the `$MYPYPATH` environment variable can be used for this.

> 3. Typeshed stubs for the standard library. These will usually be vendored by
>    type checkers, but type checkers SHOULD provide an option for users to
>    provide a path to a directory containing a custom or modified version of
>    typeshed; if this option is provided, type checkers SHOULD use this as the
>    canonical source for standard-library types in this step.

**Every mainstream checker exposes step 3**, as surveyed by the reporter in [#271](https://github.com/Nimblesite/Basilisk/issues/271).
Basilisk mirrors them — and deliberately reuses the **same names**: `typeshed-path` (kebab) is **pyrefly's** exact
spelling, and `typeshedPath` (camel, LSP JSON) is **Pyright's**.

| Checker | Step-3 option | Step-1 option | Source |
|---|---|---|---|
| **Basilisk** | `typeshed-path` / `typeshedPath` | `stub-paths` / `stubPaths` | this plan |
| **Pyright / Pylance** | `typeshedPath` | `stubPath` (default `./typings`) | [config docs](https://microsoft.github.io/pyright/#/configuration) |
| **mypy** | `custom_typeshed_dir` | `mypy_path` / `MYPYPATH` | [config docs](https://mypy.readthedocs.io/en/stable/config_file.html) |
| **pyrefly** | `typeshed-path` / `--typeshed-path` | (search path) | [docs](https://pyrefly.org/en/docs/) · [#271](https://github.com/Nimblesite/Basilisk/issues/271) |
| **ty** | `[tool.ty.environment] typeshed` | (search path) | [ty](https://github.com/astral-sh/ty) · [#271](https://github.com/Nimblesite/Basilisk/issues/271) |
| **zuban** | mypy's `--custom-typeshed-dir` | mypy's `MYPYPATH` | [#271](https://github.com/Nimblesite/Basilisk/issues/271) |

Verbatim, from the two whose docs we pulled directly:

- **Pyright** [`typeshedPath`](https://microsoft.github.io/pyright/#/configuration): *"Path to a directory that contains typeshed type stub files. Pyright ships with a bundled copy of typeshed type stubs. If you want to use a different version of typeshed stubs, you can clone the typeshed github repo to a local directory and reference the location with this path."*
- **mypy** [`custom_typeshed_dir`](https://mypy.readthedocs.io/en/stable/config_file.html): *"This specifies the directory where mypy looks for standard library typeshed stubs, instead of the typeshed that ships with mypy. … allows you to use a forked version of typeshed."*

**Layout convention** (what makes us drop-in compatible): all of these point the option at a **clone of the
[typeshed repo](https://github.com/python/typeshed)**, whose standard-library stubs live under a top-level **`stdlib/`**
directory. Basilisk therefore resolves a stdlib module as `<typeshed-path>/stdlib/<module>.pyi` — the identical on-disk
shape, so an existing Pyright/pyrefly/mypy typeshed directory works with Basilisk unchanged.

---

## Resolution-order subtlety — the actual hard part {#STUBRESPLAN-RESOLUTION-ORDER}

Jos's warning — *"pyright and ty needed some changes to resolution order"* — is the real engineering content of #271, and
it comes straight from `micropython-stdlib-stubs` being a **partial** stdlib. Two failure modes a naive "just add a
branch" implementation hits:

1. **A module absent from the custom typeshed must NOT silently resolve to the bundled one.** The spec says the custom
   directory is *"the canonical source for standard-library types in this step."* Canonical means: when `typeshed-path`
   is set, Basilisk's bundled step-3 recognition is **switched off**. Today an unresolved stdlib import is rescued
   downstream by the bundled `basilisk_stubs::is_stdlib_module` name-set (it suppresses `imports_unresolved`). If a
   custom typeshed is active, that bundled name-set MUST be bypassed (or itself sourced from the custom directory) —
   otherwise `import machine` "resolves" as a CPython stdlib name it isn't, and the override is a lie. **This is the
   resolution-order change; it is the load-bearing task below.**

2. **Precedence is fixed by the spec, and we keep it.** Step 1 (`stub-paths`) still wins over step 3
   (`typeshed-path`) — a user can still hand-patch a single stdlib module above the custom typeshed. Steps 4–5
   (site-packages) still come after. The override changes *what step 3 reads*, not *where step 3 sits*.

3. **Absent module → genuinely unresolved.** For MicroPython that is often *correct*: `import tkinter` on a board should
   be an error, not a CPython pass. Absent-from-custom-typeshed falls through steps 4–5 and, with the bundled name-set
   bypassed, ends at `Unknown` / `imports_unresolved` — the honest answer.

The full resolution flow — including this canonicality branch — is diagrammed in the spec:
[§STUBRES-RESOLUTION-FLOW](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-RESOLUTION-FLOW).

---

## Rules {#STUBRESPLAN-RULES}

- The CLI `basilisk check` path is the conformance path — any resolver change MUST keep the score from moving down.
- No `.unwrap()` / `panic!` / `unsafe`; `Result`/`Option` throughout.
- TDD: write the failing e2e test first, confirm it fails for the right reason, then implement ([CLAUDE.md](../../CLAUDE.md) Bug Fix Process).
- Keep every touched file under 500 LOC.
- Docs and code stay linked by spec ID — `grep STUBRES-` walks spec → plan → code → tests.

---

## TODO {#STUBRESPLAN-TODO}

### 1. Custom typeshed path — `typeshed-path` (step 3) {#STUBRESPLAN-TODO-TYPESHED}

Import-resolution **step 3** of the [typing spec](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering), quoted verbatim:

> Typeshed stubs for the standard library. These will usually be vendored by type
> checkers, but type checkers SHOULD provide an option for users to provide a path
> to a directory containing a custom or modified version of typeshed; if this
> option is provided, type checkers SHOULD use this as the canonical source for
> standard-library types in this step.

**Why**: [issue #271](https://github.com/Nimblesite/Basilisk/issues/271) — see [§STUBRESPLAN-REQUEST](#STUBRESPLAN-REQUEST).
**Design**: mirror pyrefly's `typeshed-path` / Pyright's `typeshedPath` ([§STUBRESPLAN-PRIOR-ART](#STUBRESPLAN-PRIOR-ART))
— point at a typeshed-layout directory, resolve `<typeshed-path>/stdlib/<module>.pyi`.
**Spec**: [§STUBRES-CUSTOM-TYPESHED](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED).

Acceptance checklist — the feature is **DONE only when every box is checked**:

- [x] **Config key** — `typeshed-path` (`pyproject.toml`, kebab) / `typeshedPath` (LSP JSON, camel) as a single `Option<PathBuf>`, resolved relative to the project root, parsed in **both** the `basilisk-config` (`BasiliskConfig`) and `basilisk-lsp` (`WorkspaceConfig`) models.
- [x] **Resolver step-3 branch** — for a stdlib module (`basilisk_stubs::is_stdlib_module`), resolve `<typeshed-path>/stdlib/<module>.pyi` **before** the bundled recognition.
- [x] **Canonicality / resolution-order fix** ([§STUBRESPLAN-RESOLUTION-ORDER](#STUBRESPLAN-RESOLUTION-ORDER)) — when `typeshed-path` is set, **bypass the bundled `is_stdlib_module` name-suppression** so a module absent from the custom typeshed falls through to steps 4–5 and, failing those, to `imports_unresolved`. The custom directory is canonical for step 3; the vendored name-set does not rescue it. **This is the load-bearing task Jos flagged.**
- [x] **Provenance** — resolved-from-custom-typeshed stubs carry `StubSource::CustomTypeshed` (Tier 1); hover reads `… (custom typeshed)` ([§STUBRESPLAN-TYPES](#STUBRESPLAN-TYPES)).
- [x] **Search-path wiring** — a `typeshed_path` field on `ImportSearchPaths`, populated by `search_paths_from_config`, set at every construction site (`workspace.rs`, test fixtures).
- [x] **e2e coverage** — custom typeshed overrides stdlib `os`; a module absent from it falls through unresolved (canonicality); non-stdlib modules are ignored by the branch; `stub-paths` (step 1) still shadows a custom typeshed; the `.pyi` actually parses (members resolve).
- [ ] **MicroPython smoke test (validate with the reporter)** — point `typeshed-path` at a real `micropython-stdlib-stubs` tree; confirm a MicroPython-specific `collections` signature type-checks and a non-MicroPython stdlib module behaves per canonicality. Invite Jos to confirm on his own project (he offered to help test).
- [x] **Docs stay in lockstep** — cross-checked [§STUBRES-CUSTOM-TYPESHED](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED), [CHKARCH-STUBS-TYPESHED](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-STUBS-TYPESHED), the [LSP shared-config table](../specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG), and website `configuration.md` (EN + ZH); no stale custom-typeshed "not implemented" note remains in those docs.
- [ ] **Verification gate** — `make test` green, conformance score **unchanged** (the conformance suite never sets `typeshed-path`, so the branch is purely additive), `make bench` within ratchet, `make lint` clean.

> **Current tree state**: the config plumbing, resolver branch, canonicality bypass, custom-typeshed provenance,
> search-path wiring, and focused e2e coverage are present in the working tree. The feature is still not shipped until
> the MicroPython smoke test and full verification gate above are complete.

### 2. Auto-stub generation {#STUBRESPLAN-TODO-AUTOGEN}

Spec: [§STUBRES-AUTOGEN](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-AUTOGEN). Today `basilisk stubs generate --all`
prints "not yet implemented" ([SPEC-CONFORMANCE-AUDIT-PLAN.md](SPEC-CONFORMANCE-AUDIT-PLAN.md) `STUBRES-AUTOGEN` row).

- [ ] `basilisk stubs generate <pkg>` — generate a Tier-3 stub for one package into `.basilisk/stubs/`.
- [ ] `basilisk stubs generate --all` — generate for every untyped import.
- [ ] `basilisk stubs status` — stub-coverage report.
- [ ] Generation modes — runtime introspection / AST inference / hybrid ([§STUBRES-AUTOGEN-MODES](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-AUTOGEN-MODES)).
- [ ] CLI parity flag — `--createstub` (Pyright-parity spelling) maps to `stubs generate`.
