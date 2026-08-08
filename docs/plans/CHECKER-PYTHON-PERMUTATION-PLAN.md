# Python Permutation Testing {#PERMTEST-PLAN}

**Objective.** Generate Python inputs that Basilisk has never seen, run them through
the checker, and decide pass/fail without a human writing an expected result. Then
measure the suite's power well enough to say when it has found most of what it can.

**Deliverable.** A permutation engine, a corpus, three oracles, and five metrics with
committed targets.

Related: [CHKARCH-TESTING-SEMANTIC-MUTATION](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING-SEMANTIC-MUTATION)
(spec), [LINESCANPLAN-SEMANTIC-MUTATION](CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md#LINESCANPLAN-SEMANTIC-MUTATION)
(deletion of what this finds).

---

## 1. Permutation families {#PERMTEST-FAMILIES}

Three families. Each carries its own oracle, so no expected-output file is authored by
hand and nothing can be fitted to a fixture.

### Family A — invariant {#PERMTEST-FAMILY-A}

Meaning preserved, text changed. **Oracle: diagnostics byte-identical.**

| Class | Permutation | Safety justification |
|---|---|---|
| A1 | Whitespace, indent width, blank lines | Discarded by the tokenizer |
| A2 | Comments added / removed / moved | Discarded by the tokenizer |
| A3 | Line breaks, continuations, redundant parens | AST-equal |
| A4 | Quote style, string prefixes on non-type strings | AST-equal |
| A5 | Trailing commas | AST-equal |
| A6 | Import aliasing (`from typing import Reversible as _R`) | Language ref: `as` binds the same object |
| A7 | Import form (`typing.SupportsIndex` ↔ `from typing import SupportsIndex`) | Same symbol, different binding path |
| A8 | Consistent alpha-rename of classes, typevars, aliases, params | Same binding structure |

A1–A5 carry a **machine-checkable precondition**: parse original and permuted, assert
ASTs equal modulo spans. A mis-specified permutation fails loudly instead of producing
a false finding. A6–A8 change the AST and are justified by binding semantics, so they
assert on resolved symbol identity instead.

### Family B — directed {#PERMTEST-FAMILY-B}

Meaning changed in a known direction. **Oracle: a specific diagnostic must appear or
disappear.** Catches missing detection and false positives, which Family A cannot.

| Class | Permutation | Expected delta |
|---|---|---|
| B1 | Assign a value of an incompatible type | assignment diagnostic appears |
| B2 | Repair a known-bad line | that diagnostic disappears, no others move |
| B3 | Widen an annotation (`SupportsIndex` → `object`) | error clears or stays clear |
| B4 | Narrow an annotation (`object` → `Reversible[str]`) | error appears or stays present |
| B5 | Drop a required argument / add an extra one | arity diagnostic appears |
| B6 | Remove a decorator the rule depends on | the dependent diagnostic moves as specified |
| B7 | Add a branch made unreachable by a narrowing | reachability diagnostic appears |

Each class states its expected delta as a **relation** (appears / disappears / unchanged),
never as a literal diagnostic list. Relations survive rule renumbering and message edits.

### Family C — differential {#PERMTEST-FAMILY-C}

Same permuted input to pyright, mypy, pyrefly, ty, zuban. **Oracle: disagreement is a
triage item, not a failure.** Sanctioned by [CLAUDE.md](../../CLAUDE.md) — compare
against, never copy.

- Unanimous others vs. Basilisk → high-priority triage.
- Split field → recorded, no action implied.
- Never auto-adopt another checker's verdict; every resolution cites the typing spec.

---

## 2. Symbol vocabulary — the binding constraint {#PERMTEST-VOCABULARY}

**Every authored example uses symbols the conformance suite does not contain.** The
checker carries a hardcoded arm for the suite's vocabulary
([`CONFORMANCE-SPELLING-CHEAT-INVENTORY.md`](../CONFORMANCE-SPELLING-CHEAT-INVENTORY.md)),
so a test written in that vocabulary passes whether the rule resolves symbols or greps
for them. Those spellings caused the defect; they cannot be the material used to detect
it. This constraint outranks convenience in every other section of this plan.

### The two vocabularies {#PERMTEST-VOCABULARY-SETS}

Both derived mechanically from the **freshly cloned** suite —
`conformance/tests/` is gitignored, so the lists are regenerated per run and never
committed. Measured at time of writing:

| Vocabulary | Size | Rule |
|---|---|---|
| Identifiers the suite defines (`class` / `def` names) | 913 | **Banned** in authored examples |
| `typing`/`typing_extensions` symbols the suite imports | 55 | **Quarantined** — see below |
| `typing.__all__` symbols the suite never touches | **53 of 105** | The required pool |

```bash
# Banned identifiers
grep -rhoE "^(class|def) [A-Za-z_][A-Za-z0-9_]*" conformance/tests/*.py \
  | awk '{print $2}' | sort -u                                   # 913

# Suite typing vocabulary (quarantined)
grep -rhoE "^from (typing|typing_extensions) import .*" conformance/tests/*.py \
  | sed 's/^from [a-z_]* import //' | tr ',' '\n' | sed 's/ as .*//' \
  | tr -d ' ()' | sort -u                                        # 55
```

The out-of-vocabulary pool at time of writing — none of these appear anywhere in the
suite, so **no hardcoded arm can exist for them**:

> `AbstractSet`, `AnyStr`, `AsyncContextManager`, `AsyncGenerator`, `AsyncIterable`,
> `AsyncIterator`, `Awaitable`, `BinaryIO`, `ChainMap`, `Container`, `ContextManager`,
> `Counter`, `Deque`, `ForwardRef`, `Generator`, `ItemsView`, `KeysView`, `MappingView`,
> `Match`, `MutableMapping`, `MutableSequence`, `MutableSet`, `NoDefault`, `OrderedDict`,
> `ParamSpecArgs`, `ParamSpecKwargs`, `Pattern`, `Reversible`, `SupportsAbs`,
> `SupportsBytes`, `SupportsComplex`, `SupportsFloat`, `SupportsIndex`, `SupportsInt`,
> `SupportsRound`, `Text`, `TextIO`, `ValuesView`, `clear_overloads`,
> `evaluate_forward_ref`, `get_args`, `get_origin`, `get_overloads`,
> `get_protocol_members`, `get_type_hints`, `is_protocol`, `is_typeddict`,
> `no_type_check_decorator`, … (list regenerated per run)

Extend the pool with stdlib surfaces the suite never exercises — `contextlib`,
`weakref`, `functools.cached_property`, `collections.abc` breadth, `array`, `ctypes`,
`enum` variants.

### The three rules {#PERMTEST-VOCABULARY-RULES}

1. **Identifiers.** No authored or generated example may use any of the 913 banned
   names. Generated names come from a namespace disjoint from the suite by
   construction, not by inspection.
2. **Library symbols.** Every rule's test set must contain **at least one case built on
   an out-of-vocabulary symbol**, wherever the concept admits one. A generics rule
   tested only through `TypeVar` is unverified; the same rule reached through
   `SupportsIndex` or `Reversible` is a real test of the mechanism.
3. **Unavoidable symbols are quarantined, not exempt.** Some concepts have exactly one
   spelling — there is only one `TypeVar`. For those, aliasing (A6) and import-form
   (A7) permutations are **mandatory, not optional**, and a case using the bare
   in-vocabulary spelling never counts toward coverage on its own.

- [ ] Script the two derivations above; run against the fresh clone, never a cached copy.
- [ ] CI check: any authored fixture using a banned identifier fails the build.
- [ ] Quarantine check: a rule whose entire test set is in-vocabulary and un-aliased is
  reported as **unverified**, not as covered.

**Scope.** These rules bind Tier C1 and C2 (authored and generated). Tier C3 is real
third-party Python and will incidentally reuse common names; there the overlap is
**measured, not banned**.

---

## 3. Corpus {#PERMTEST-CORPUS}

Permutations need input to permute. Three tiers, in build order:

| Tier | Source | Purpose |
|---|---|---|
| C1 | Existing rule tests in `crates/basilisk-checker/tests`, **alpha-renamed out of the banned vocabulary** | Cheapest start; every rule already has entry points |
| C2 | Generated from the typing-spec grammar — each construct in each scope context, names drawn from the disjoint namespace, symbols preferring the out-of-vocabulary pool | Reaches constructs and symbols no test file contains |
| C3 | Real Python: top PyPI packages by download, pinned by version + SHA | Input diversity no one authors by hand |

**`conformance/tests/` is not a corpus tier.** Permuting the artefact the code was
fitted to measures nothing. Its only role in this plan is as the **source of the
banlist** ([PERMTEST-VOCABULARY](#PERMTEST-VOCABULARY)).

- [ ] C1 wired first — it is the fastest path to a non-zero grid. Renaming its fixtures
  out of the banned vocabulary is itself an A8 permutation, so it costs nothing extra
  and immediately exposes any rule keyed to a suite identifier.
- [ ] C2 generated from the construct list, not hand-written, with symbol selection
  biased to the out-of-vocabulary pool.
- [ ] C3 pinned by SHA and vendored-by-reference; a floating corpus makes runs
  non-reproducible and metric movement unattributable.

---

## 3. Metrics {#PERMTEST-METRICS}

Six. Each has a definition, a computation, a target, and the failure mode it guards.
M2 is the primary answer to *are we there yet*; M5 is the one that keeps M1 honest.

### M1 — Grid fill {#PERMTEST-M1}

**Definition.** Fraction of (rule × permutation class) cells actually exercised.
**Computed.** Cells run / (rules × classes). Denominator is fixed and published.
**Target.** 100%, with the uncovered remainder **named** at every report.
**Guards.** A score over a hand-picked subset reading as a complete measure.

### M2 — Seeded-defect kill rate {#PERMTEST-M2}

**The metric that answers "is the suite strong enough."** Plant defects of the exact
class we are hunting, then measure what fraction the suite catches.

**Definition.** Of N deliberately injected defects, the fraction the permutation suite
fails on.
**Computed.** A seed catalogue of scripted source patches, each re-introducing a known
defect class — replace a resolved-symbol lookup with `== "TypeVar"`; make a check
depend on identifier case; make a branch depend on a specific spacing; hardcode a
builtin name as a prefix. Apply one at a time, run the suite, record caught/missed.
**Target.** ≥95% caught, and **100% on the spelling-dependence classes**, which are the
defects the audit found and the ones the suite exists for.
**Guards.** A green suite that is green because it is weak. This is the only metric that
distinguishes "no bugs left" from "no bugs detectable."

Seeds are generated by patching the tree, never committed to it. The catalogue is
derived from [`CONFORMANCE-SPELLING-CHEAT-INVENTORY.md`](../CONFORMANCE-SPELLING-CHEAT-INVENTORY.md)
cheat classes S1–S5, so every historically-real defect shape is represented.

### M3 — Construct coverage {#PERMTEST-M3}

**Definition.** Fraction of Python constructs the corpus contains at all.
**Computed.** Enumerate `ruff_python_parser` node kinds × scope context (module, class
body, function body, nested function, comprehension, `TYPE_CHECKING` block). Count
which cells any corpus file reaches.
**Target.** Ratchet upward; publish the uncovered cell list.
**Guards.** A permutation engine running at full tilt over input that never contains
the construct in question. Bounded by the grammar, so it terminates.

### M4 — Discovery curve {#PERMTEST-M4}

**Definition.** New unique failures per 10× corpus growth.
**Computed.** Dedupe failures by (rule, permutation class, minimised repro). Plot
against corpus size across runs.
**Target.** Saturation — a 10× corpus increase yielding **zero** new unique failures,
sustained across two consecutive expansions.
**Guards.** Declaring completion at an arbitrary corpus size. This is the empirical
"enough" signal, and it is only trustworthy when M2 is already high; a weak suite
saturates immediately and means nothing.

### M5 — Out-of-vocabulary coverage {#PERMTEST-M5}

**Definition.** Fraction of rules whose test set contains at least one case built on a
symbol and identifier set disjoint from the conformance vocabulary.
**Computed.** Per rule: does any case use only non-banned identifiers **and** at least
one out-of-vocabulary library symbol (or, for quarantined concepts, an aliased/alternate
import form)? Rules meeting neither condition are counted **unverified**.
**Target.** 100%. Banned-identifier usage: 0, CI-enforced.
**Guards.** The exact failure that produced the audit — a rule that looks covered
because its test is written in the vocabulary the rule hardcodes. A high M1 with a low
M5 means the grid is full of tests that cannot discriminate.

### M6 — Rule-layer source access {#PERMTEST-M6}

**Definition.** Sites where a rule can read raw source text.
**Computed.** Compiler-verified, by making `ResolvedModule::source`
([`resolved_module.rs:321`](../../crates/basilisk-resolver/src/scope/resolved_module.rs#L321))
private and counting errors. Grep floor today: 74 `.source`, 93 `slice_span`, 72
`source: &str` under `crates/basilisk-checker/src/rules`.
**Target.** 0, ratcheted, lint-enforced across all crates.
**Guards.** Re-growth. A rule that cannot see spelling cannot depend on it, which
removes the whole Family A defect class by construction rather than by sampling.

---

## 4. Phases {#PERMTEST-PHASES}

### Phase 1 — engine and Family A over C1

- [ ] Permutation engine: input file → set of permuted files, each tagged with class.
- [ ] AST-equality precondition asserted for A1–A5.
- [ ] Family A oracle: byte-identical diagnostics, diff reported on failure.
- [ ] Run over C1. **Expect failures; they are the deliverable.** Each is disposed of
  by failing test → delete → report
  ([CHKARCH-TEXT-MATCHED-LOGIC](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TEXT-MATCHED-LOGIC)),
  never by teaching the harness to tolerate the difference.
- [ ] Derive the banlist and out-of-vocabulary pool from the fresh clone; wire the
  CI banned-identifier check **before** any fixture is authored.
- [ ] Alpha-rename C1 fixtures out of the banned vocabulary as the first permutation run.
- [ ] Report M1, M5 and M6 from this phase.

### Phase 2 — seed catalogue and M2

- [ ] Build the seed catalogue from cheat classes S1–S5.
- [ ] Include seeds keyed to **suite-vocabulary spellings specifically** — a planted
  `== "TypeVar"` must be caught by an out-of-vocabulary or aliased case, which is the
  direct test that M5 is doing its job.
- [ ] Run seeded defects through the Phase 1 suite; publish the first M2.
- [ ] **If M2 on spelling classes is below 100%, Phase 1 is not done.** Strengthen the
  suite until it is; do not proceed on an unmeasured suite.

### Phase 3 — Family B and C2

- [ ] Directed permutations with relation-based oracles.
- [ ] Grammar-generated corpus; report M3.
- [ ] Extend the seed catalogue to detection defects (dropped diagnostics, inverted
  conditions) and re-measure M2.

### Phase 4 — C3 and saturation

- [ ] Pinned PyPI corpus; automatic failure minimisation to a small repro.
- [ ] Family C differential comparison with triage queue.
- [ ] Report M4 across corpus expansions until saturation.

### Phase 5 — gate

- [ ] Fast subset in `make test`; full run nightly.
- [ ] Ratchet M1, M2, M3, M5, M6. **M4 is a report, not a gate** — gating on a discovery
  curve rewards a weak suite that finds nothing.

---

## 5. Rules of engagement {#PERMTEST-ROE}

- **Never author an example in the conformance vocabulary.** Reaching for `TypeVar`,
  `Protocol`, or a suite identifier because it is familiar is how this happened; the
  out-of-vocabulary pool is the default, and in-vocabulary spellings are quarantined
  ([PERMTEST-VOCABULARY](#PERMTEST-VOCABULARY)).
- **Never mutate an expected result to match observed output.** The expectation is the
  unpermuted run; only the input varies.
- **Never weaken a permutation class** to make a run green. A failing permutation is a
  finding about the checker.
- **Every metric is published with its denominator** and its uncovered remainder.
- **No conformance figure** is quoted or targeted here
  ([CHKARCH-CONFORMANCE](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFORMANCE)).
  Drops caused by deletions this suite triggers are recorded, never reverted.
- **`cargo-mutants` is unrelated.** It mutates Rust and measures test sensitivity to
  Rust change. It cannot detect spelling dependence, and its kill rate is never quoted
  without its denominator and exclusions.

---

## 6. Acceptance {#PERMTEST-ACCEPTANCE}

| Metric | Target |
|---|---|
| M1 grid fill | 100%, remainder named |
| M2 seeded-defect kill rate | ≥95% overall, 100% on spelling classes |
| M3 construct coverage | ratcheted, uncovered cells published |
| M4 discovery curve | saturated across two consecutive 10× expansions |
| M5 out-of-vocabulary coverage | 100% of rules; 0 banned identifiers, CI-enforced |
| M6 rule-layer source access | 0, lint-enforced |

Plus: every permutation class cites the language-reference clause justifying it; the
seed catalogue covers S1–S5; failures found by this suite are on the record together
with what was deleted in response.

**No rule is reported as verified on the strength of an in-vocabulary test.** A rule
exercised only through the 55 symbols and 913 identifiers the conformance suite
contains is reported **unverified**, whatever its other metrics say.
