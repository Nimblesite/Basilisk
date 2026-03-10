# PEP Conformance

## What it is

PEP conformance is a **percentage score** measuring how accurately Basilisk
implements the Python typing specification.  It is produced by running the
official `python/typing` conformance test suite — the same suite used to score
Pyright, mypy, Pyrefly, and every other type checker.

The suite lives at:
<https://github.com/python/typing/tree/main/conformance>

Each test file covers one narrow behaviour defined in a typing PEP.  Lines
that must produce a type error are marked `# E`.  Lines where an error is
optional are marked `# E?`.  Lines that belong to a mutually-exclusive group
are marked `# E[tag]`.

A file **passes** when Basilisk emits at least one diagnostic on every `# E`
line.  The overall score is `passing_files / total_files × 100`.

---

## Why it matters

Every competing tool claims conformance.  None of them agree on what that
means unless it is measured against the same test suite.  Without this suite,
"95% conformance" is marketing copy.  With it, it is a fact you can verify on
your own machine in under a minute.

Official scores from the [python/typing conformance suite results](https://github.com/python/typing/blob/main/conformance/results/results.html)
(versions tested: pyright 1.1.408, mypy 1.19.1, zuban 0.6.1, pyrefly 0.54.0):

| Tool     | Full-pass | Partial+pass | Measured by             |
|----------|-----------|--------------|-------------------------|
| Pyright  | ~99%      | 100%         | python/typing suite     |
| Zuban    | ~98%      | 100%         | python/typing suite     |
| Pyrefly  | ~86%      | 100%         | python/typing suite     |
| mypy     | ~58%      | ~96%         | python/typing suite     |
| ty       | alpha — not yet in suite | — | [independent blog, Aug 2025](https://sinon.github.io/future-python-type-checkers/) |
| Basilisk | **run the harness**      | — | python/typing suite     |

The target for Basilisk is **95%+ full-pass** — matching Pyright, the current best-in-
class, while being strict-by-default and not requiring a Node.js runtime.

---

## How to measure it right now

```bash
# 1. Download the suite (one-time, not committed to the repo)
./scripts/fetch-conformance.sh

# 2. Run the harness
cargo test --test conformance_tests -- --nocapture
```

The output is a scorecard broken down by PEP category. This is an example only. This is not the current state of the scorecard.

```
╔══════════════════════════════════════════════════════════════╗
║           BASILISK PEP CONFORMANCE SCORECARD                 ║
╠══════════════════════════════════════════════════════════════╣
║  Files:     145 total │   86 pass │   59 fail            ║
║  Score:    59.3%                                           ║
║  Required:  343 caught │  595 missed                       ║
║  Tagged:     13 groups ok │   46 groups missed              ║
║  False+:     39 unexpected diagnostics                       ║
╠══════════════════════════════════════════════════════════════╣
║  Category breakdown                                          ║
╠══════════════════════════════════════════════════════════════╣
║                          7/7   100.0%  ████████████████████  ║
║  aliases                 1/7    14.3%  ███░░░░░░░░░░░░░░░░░  ║
║  annotations             4/5    80.0%  ████████████████░░░░  ║
║  callables               0/4     0.0%  ░░░░░░░░░░░░░░░░░░░░  ║
║  classes                 1/2    50.0%  ██████████░░░░░░░░░░  ║
║  constructors            3/6    50.0%  ██████████░░░░░░░░░░  ║
║  dataclasses            11/16   68.8%  ██████████████░░░░░░  ║
║  directives              9/10   90.0%  ██████████████████░░  ║
║  enums                   6/6   100.0%  ████████████████████  ║
║  exceptions              1/1   100.0%  ████████████████████  ║
║  generics               14/30   46.7%  █████████░░░░░░░░░░░  ║
║  historical              1/1   100.0%  ████████████████████  ║
║  literals                1/4    25.0%  █████░░░░░░░░░░░░░░░  ║
║  namedtuples             1/4    25.0%  █████░░░░░░░░░░░░░░░  ║
║  narrowing               0/2     0.0%  ░░░░░░░░░░░░░░░░░░░░  ║
║  overloads               4/4   100.0%  ████████████████████  ║
║  protocols               3/11   27.3%  █████░░░░░░░░░░░░░░░  ║
║  qualifiers              3/3   100.0%  ████████████████████  ║
║  specialtypes            4/5    80.0%  ████████████████░░░░  ║
║  tuples                  2/3    66.7%  █████████████░░░░░░░  ║
║  typeddicts             10/14   71.4%  ██████████████░░░░░░  ║
╠══════════════════════════════════════════════════════════════╣

```

This is the **Phase 1 baseline** measured 2026-02-28.

The score is **informational** — the test never hard-fails due to a low score.
It fails only if the conformance directory is missing (setup problem).

---

## What the score means today

Basilisk is in **Phase 2**.  The checker implements 25 rules covering:

The score has improved significantly, showing good progress in Phase 2 implementation.

---

## Road to 100%

The conformance suite files group naturally into implementation phases.  The
table below maps each category to the PEP that governs it and the estimated
implementation phase.

### Phase 2 — Core typing mechanics (target: ~40%)

These are the foundational PEPs.  Without them nothing else works.

| Category | PEP | Key behaviours |
|---|---|---|
| `specialtypes_*` | 484 | `Any`, `Never`, `None`, promotions, `type[]` |
| `annotations_*` | 484, 526, 563 | Forward refs, `from __future__ import annotations`, generators, coroutines |
| `aliases_*` | 484, 613, 695 | `TypeAlias`, `type` statement, `NewType`, recursive aliases |
| `qualifiers_*` | 591, 681 | `Final`, `ClassVar`, `Annotated` |
| `literals_*` | 586, 675 | `Literal`, `LiteralString` |
| `directives_*` | 484 | `cast`, `assert_type`, `reveal_type`, `type: ignore`, `TYPE_CHECKING` |

### Phase 3 — Classes and generics (target: ~65%)

| Category | PEP | Key behaviours |
|---|---|---|
| `generics_basic*`, `generics_variance*`, `generics_upper_bound*` | 484, 695 | TypeVar, bounds, variance, `type T = ...` syntax |
| `generics_self_*` | 673 | `Self` type |
| `generics_defaults*` | 696 | TypeVar defaults |
| `classes_*` | 484, 591, 698 | `ClassVar`, `@override` |
| `constructors_*` | 484 | `__init__`, `__new__`, `__class_getitem__` |
| `tuples_*` | 484, 646 | Fixed-length tuples, `Unpack` |
| `namedtuples_*` | 484 | `NamedTuple` |

### Phase 4 — Advanced generics and protocols (target: ~80%)

| Category | PEP | Key behaviours |
|---|---|---|
| `protocols_*` | 544 | Structural subtyping, runtime-checkable, variance, generic protocols |
| `callables_*` | 484, 612 | `Callable`, `ParamSpec`, `Concatenate`, kwargs |
| `generics_paramspec_*` | 612 | `ParamSpec` components, semantics, specialization |
| `generics_typevartuple_*` | 646 | Variadic generics, `Unpack`, `TypeVarTuple` |
| `generics_syntax_*` | 695 | PEP 695 type parameter syntax, scoping, variance inference |

### Phase 5 — Structural types and overloads (target: ~90%)

| Category | PEP | Key behaviours |
|---|---|---|
| `overloads_*` | 484 | Overload consistency, evaluation, stub definitions |
| `typeddicts_*` | 589, 692, 705 | Class/functional syntax, inheritance, `Required`/`NotRequired`, `ReadOnly`, `**kwargs` |
| `dataclasses_*` | 681 | `@dataclass`, `@dataclass_transform`, frozen, slots, `__post_init__` |
| `enums_*` | 484 | `Enum` member types, expansion, behaviours |
| `exceptions_*` | 484 | `ExceptionGroup`, context managers |

### Phase 6 — Narrowing and edge cases (target: ~95%)

| Category | PEP | Key behaviours |
|---|---|---|
| `narrowing_*` | 647, 742 | `TypeGuard`, `TypeIs` (exhaustive narrowing) |
| `generics_scoping*`, `generics_type_erasure*` | 695 | Scope rules, type erasure |
| `historical_positional*` | 570 | Positional-only parameters |
| All `# E?` optional lines | various | Optional errors where Basilisk currently over- or under-fires |

---

## Scoring rules (technical)

The harness in `crates/basilisk-cli/tests/conformance_tests.rs` implements the
annotation format from the `python/typing` conformance spec:

| Annotation  | Rule |
|-------------|------|
| `# E`       | Basilisk MUST emit at least one diagnostic on this line |
| `# E?`      | Basilisk MAY emit a diagnostic — tracked but not required |
| `# E[tag]`  | Exactly one line in the group must have a diagnostic |
| `# E[tag+]` | One or more lines in the group may have a diagnostic |

A file **passes** when every `# E` line has at least one diagnostic.

False positives (Basilisk fires on an unmarked line) are counted and reported
but do not fail a file.  A high false-positive rate signals that strict-by-
default rules are firing on code the conformance suite considers valid —
this is expected in Phase 1 and will be addressed in Phase 2 when context-
aware analysis suppresses annotations-required rules inside conformance
fixtures.

---

## Keeping the score honest

- The conformance files are **not committed** to this repo.  They are
  downloaded from the upstream `python/typing` repository by
  `./scripts/fetch-conformance.sh`.  This prevents the score from being
  gamed by modifying the tests.

- The `COMMIT` variable in `scripts/fetch-conformance.sh` pins the suite to
  a specific ref.  Update it deliberately when pulling upstream changes so
  score movements are intentional, not accidental.

- The harness test never hard-fails on a low score.  Score regressions are
  caught by tracking the number in CI and alerting on drops.  Adding a
  hard floor (e.g. `assert!(pct >= 40.0)`) is appropriate once Phase 2 lands.
