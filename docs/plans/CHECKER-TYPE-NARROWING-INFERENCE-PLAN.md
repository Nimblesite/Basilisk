# Type Narrowing and Inference Plan {#NARROWPLAN-INFERENCE}

Specs: [TYPEINF-OVERVIEW](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-OVERVIEW),
[TYPEINF-TARGET](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET), and
[CHKARCH-INFERENCE](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INFERENCE).

Basilisk already passes the typing conformance suite. This plan therefore has two
tracks that share one implementation:

1. **Consolidation** — merge duplicated rule-local inference into shared
   components and improve editor/user behavior without weakening the
   zero-false-positive gate.
2. **A substantially more powerful inference engine** — bidirectional
   (synthesis + checking) typing over a subtype-constraint solver, per the
   target architecture in
   [TYPEINF-TARGET](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET)
   and the research grounding in
   [TYPEINF-RESEARCH](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-RESEARCH).
   This closes inference-gap issues like
   [#317](https://github.com/MelbourneDeveloper/Basilisk/issues/317) and lays
   the groundwork for PEP 827-style type manipulation.

Current foundations include `InferredType`, annotation parsing, literal RHS
inference, resolver-collected narrowing guards, and several rule-local
Protocol/TypedDict/Callable algorithms. There is no shared checker-side flow
environment, expression inferrer, constraint solver, or subtype context.

## Goals and non-goals {#NARROWPLAN-GOALS}

**Goals**

- Inference powerful enough that annotations become recoverable from usage.
  Issue [#317](https://github.com/MelbourneDeveloper/Basilisk/issues/317) is
  the canonical example: in `def multiply(x, y) -> int: return x * y` called as
  `multiply(4, 5)`, both parameters are recoverable from the body constraint
  (`x * y` must produce `int` given `-> int`, via constraint-based inference
  over the overloaded `__mul__`/`__rmul__` operator) and from the call site.
  A constraint-based and/or call-site-driven engine types this with no
  annotations, making a `BSK-0001` diagnostic unnecessary in such cases.
- Everything is oriented toward
  [PEP 827 – Type Manipulation](https://peps.python.org/pep-0827/): the engine
  must become powerful enough (bidirectional context, constraint solving,
  bounded type-level evaluation) that PEP 827's conditional/mapped types have a
  sound home if adopted later.
- Preserve the gradual guarantee as a testable invariant, keep the
  zero-false-positive conformance gate, and hold both benchmark ratchets
  ([CHKARCH-TESTING-BENCH-RATCHET](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING-BENCH-RATCHET)).
- Beat competing checkers on the measurable axes in
  [NARROWPLAN-TARGETS](#NARROWPLAN-TARGETS).

**Non-goals**

- **Not implementing PEP 827.** No `typing.IsAssignable`, Type Booleans,
  conditional type expressions, or `RaiseError` surface syntax ships from this
  plan. The type-level evaluation work in
  [NARROWPLAN-TYPELEVEL](#NARROWPLAN-TYPELEVEL) is engine groundwork
  (bounded normalization, memoization, divergence fallbacks) so the inference
  engine is PEP 827-ready — nothing more.
- No alternate checking mode
  ([CHKARCH-CONFIGURATION-ONLY](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIGURATION-ONLY));
  every shared component lands behind existing checker APIs.
- No global Hindley–Milner/Algorithm W core — see
  [TYPEINF-RESEARCH-THEORY](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-RESEARCH-THEORY)
  for why unification-based HM does not fit Python's subtyping, mutation, and
  gradual `Any`.

## Staged delivery {#NARROWPLAN-STAGES}

Delivery is staged so each stage is independently shippable and each carries an
explicit decision threshold for changing course.

- **Stage 0 — foundational commitments** (before writing the solver): the
  two-mode bidirectional core and the two-stage constraint architecture with
  deferred generalization, per
  [TYPEINF-TARGET-BIDIRECTIONAL](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-BIDIRECTIONAL)
  and
  [TYPEINF-TARGET-CONSTRAINTS](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-CONSTRAINTS).
  *Threshold:* if bidirectional `check` mode cannot be threaded cleanly through
  Salsa queries without ballooning dependencies, fall back to Pyrefly-style
  "peek-ahead" only for the specific constructs that fail.
- **Stage 1 — incrementality**: match ty on granularity, exceed Pyrefly, per
  [TYPEINF-TARGET-INCREMENTAL](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-INCREMENTAL).
  *Threshold:* if fine-grained queries cause memory blowup on a 1M+ LOC
  target, adopt Pyrefly-style AST/binding eviction (drop intermediate state,
  keep only interfaces) behind the query layer.
- **Stage 2 — flow-sensitive narrowing that beats everyone**, per
  [TYPEINF-TARGET-NARROWING](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-NARROWING),
  together with the consolidation sections below.
- **Stage 3 — type-level computation groundwork for PEP 827 readiness**, per
  [TYPEINF-TARGET-TYPELEVEL](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-TYPELEVEL).

## Flow analysis {#NARROWPLAN-FLOW}

Add a scoped narrowing environment with branch push, complement, join, and
nested-function boundaries, consuming resolver guards (`isinstance`, `is None`,
truthiness, `TypeGuard`, `TypeIs`, `assert`, pattern matching). Model
assignment narrowing without changing the declared type used for assignment
validation.

The target formulation is occurrence typing
(Tobin-Hochstadt–Felleisen; Castagna et al. — see
[TYPEINF-RESEARCH-GRADUAL](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-RESEARCH-GRADUAL))
as **intersection-and-negation-based narrowing** (ty's model) over a
Salsa-backed use-def map, with `phi`/join at control-flow merges (Pyrefly's
binding DSL). Supported guards grow to include `issubclass`, `==`/`in` against
literals, `hasattr` (synthetic protocol intersection), and
exhaustiveness/implied-else. The **attribute-narrowing-across-calls** soundness
tradeoff is decided explicitly per Pyrefly's lesson — default to Pyrefly's
usable behavior but make it configurable for security-sensitive users.
Reachability becomes **inference-driven** (ty's model) rather than
pattern-matched idioms.

## Expression inference {#NARROWPLAN-EXPRESSIONS}

Infer same-module and imported function/method return types; constructor,
attribute, subscript, binary/unary, conditional, and walrus expressions from
structured AST data. Centralize builtin constructor/method signatures instead
of adding rule-local string tables, and reuse the same inference results for
diagnostics, hover, completions, and inlay hints.

With the bidirectional core in place, `check` mode propagates expected types
into container literals, comprehensions, lambda parameters, and call
arguments; parameter types become recoverable from body constraints and call
sites (issue #317).

## Generic constraints {#NARROWPLAN-CONSTRAINTS}

Collect lower, upper, constrained, default, and expected-return bounds for
TypeVars; solve bounds deterministically and report ambiguity without
guessing. Cover constrained/bound TypeVars, PEP 696 defaults, ParamSpec, and
TypeVarTuple interactions before wiring the solver into rule decisions.

Type variables carry explicit lower/upper bounds (like Pyright's type
intervals and Pyrefly's `Var`) with the input/output polarity discipline
borrowed from Dolan/Parreaux — **without** committing to full biunification
(see the risk in [NARROWPLAN-RISKS](#NARROWPLAN-RISKS)). Generalization is
deferred: infer `list[Var{lower=Literal[1]}]` and settle `Var` only at first
constraining use, preserving `list[int]` vs `list[Literal[1]]` precision
instead of eagerly widening `Literal[1] → int`. The Jane Street Q&A confirms
Pyrefly's own team conceded the deferred "most general type" is "the correct
type" and "might be more enjoyable" — Basilisk should ship it.

## Shared subtyping {#NARROWPLAN-SUBTYPING}

Build a context for nominal class relationships, Protocol members, TypedDict
schemas, generic variance, and Callable parameter kinds. Replace duplicated
rule-local subtype helpers only after parity tests pin their current
accepted/rejected cases. Keep `Any`/`Unknown` gradual behavior and the numeric
tower consistent across annotation parsing and inferred types.

## Incrementality {#NARROWPLAN-INCREMENTAL}

Use Salsa with **definition-level and expression-level tracked queries** (ty's
model), not file-level (Pyrefly's). Compute a compact per-module
**interface/signature** query as the cross-file dependency boundary (Pyrefly's
"Interface" idea) to get early cutoff and prevent whole-program invalidation.
Model cycles with **fixpoint iteration seeded by a divergent/bottom sentinel**
and a hard iteration cap (ty's `Divergent`; Pyrefly's thunks). The Stage 1
memory threshold in [NARROWPLAN-STAGES](#NARROWPLAN-STAGES) governs the
eviction fallback.

## Type-level evaluation groundwork (PEP 827 readiness) {#NARROWPLAN-TYPELEVEL}

Build a **normalization-by-evaluation engine** for type-level functions
(conditional/mapped types) as a set of memoized Salsa queries returning types
in weak-head normal form (whnf). Enforce:

- (a) **fuel/depth bounds** (TypeScript's instantiation-depth model);
- (b) **memoization** of normalized results;
- (c) a **`Divergent`/`@Todo` fallback** that preserves the gradual guarantee
  when evaluation is truncated;
- (d) **GHC-style acceptance conditions** (Paterson/Coverage analogues) that
  statically reject obviously-nonterminating type-level definitions, with an
  opt-in "undecidable" escape hatch.

Represent mapped types as **kind `Type → Type` operators** and conditional
types as guarded rewrites keyed on a consistency/assignability check
(`IsAssignable` in PEP 827); evaluate lazily (call-by-need) so unused branches
never diverge. Bounded evaluation is mandatory, not optional: type-level
computation in this space is provably Turing-complete (see
[TYPEINF-RESEARCH-TYPELEVEL](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-RESEARCH-TYPELEVEL)).

Scope reminder: this is **engine groundwork only** — no PEP 827 surface
feature ships from this plan (see [NARROWPLAN-GOALS](#NARROWPLAN-GOALS)).

## Integration {#NARROWPLAN-INTEGRATION}

Introduce each shared component behind existing checker APIs; do not create an
alternate checking mode. Migrate assignment, return, call, and `assert_type`
rules incrementally, deleting the replaced local logic in the same change. Add
spec-ID-linked mutation-resistant tests for each migrated behavior.

## Measurable targets {#NARROWPLAN-TARGETS}

Dimensions on which Basilisk can beat Pyrefly, each with its measurement:

- **Bidirectional literal/generic inference:** deferred bounded type variables
  preserve `list[int]` vs `list[Literal[1]]` precision *and* accept more
  programs (measure against the typing-spec conformance suite + a curated
  container/comprehension/lambda benchmark).
- **Narrowing richness:** intersection+negation narrowing and
  `hasattr`/pattern narrowing (measure against the utahplt/ifT-benchmark,
  <https://github.com/utahplt/ift-benchmark>).
- **Higher-order inference:** propagate expected types through
  `map`/`filter`/decorators/`ParamSpec` (build a targeted higher-order
  benchmark; Pyrefly is strong here, so this is the hardest win).
- **Incremental latency:** aim for single-digit-millisecond keystroke updates
  via definition-level Salsa queries (ty reports 4.7ms on a load-bearing
  PyTorch edit) vs Pyrefly's file-level invalidation (measure p50/p99 re-check
  latency on a 1M-LOC corpus).
- **Gradual-guarantee conformance:** a differential test that strips
  annotations and asserts no new errors — Pyrefly fails this by design;
  Basilisk should pass.

## Risks and decision thresholds {#NARROWPLAN-RISKS}

- **Decidability wall.** With PEP 827 conditional/mapped types, type-level
  evaluation is Turing-complete (proven for both TypeScript and Python type
  hints — see
  [TYPEINF-RESEARCH-TYPELEVEL](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-RESEARCH-TYPELEVEL)).
  Basilisk cannot be simultaneously complete and terminating; bounded
  evaluation with explicit `Divergent`/`@Todo` fallbacks is mandatory, and
  some legitimate type-level programs will hit the bound. This is an inherent
  limitation, not an implementation gap.
- **Algebraic subtyping may not transfer.** MLsub/Simple-sub assume
  structural, fully-inferred types; Python's nominal classes, invariant
  generics, overloads, and `Any` do not fit cleanly. Committing to full
  biunification is a research risk; the safe path is to borrow polar types and
  constraint simplification only. Flag this as unvalidated until a prototype
  confirms it on real typeshed stubs.
- **Soundness vs. practicality is unavoidable.** Pyrefly explicitly chose
  usability over soundness (attribute narrowing across calls). Every such
  choice trades false negatives for ergonomics. Basilisk must make these
  choices *explicitly and configurably*, and document them, rather than
  inheriting them implicitly.
- **Gradual guarantee vs. aggressive inference are in direct tension.** You
  cannot both (a) infer concrete types in unannotated code to catch bugs
  (Pyrefly) and (b) guarantee that removing annotations never adds errors
  (ty). Basilisk must pick a default and likely offer a strictness dial;
  claiming to "beat Pyrefly on inference" while "preserving the gradual
  guarantee" requires being precise about *which* mode is being compared.
- **Incrementality vs. global inference.** The more Basilisk infers across
  function/call boundaries (to beat Pyrefly), the larger its Salsa dependency
  graph and the more a single edit invalidates. The interface/signature-boundary
  technique mitigates but does not eliminate this; expect to tune the
  granularity empirically.
- **Stage thresholds.** The per-stage fallback triggers live in
  [NARROWPLAN-STAGES](#NARROWPLAN-STAGES): peek-ahead fallback if `check` mode
  balloons Salsa dependencies; eviction fallback if fine-grained queries blow
  memory on a 1M+ LOC target.
- **Source quality of competitor numbers.** Comparative figures quoted in
  [TYPEINF-RESEARCH-COMPETITORS](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-RESEARCH-COMPETITORS)
  are vendor/benchmark claims, not independently audited; treat them as
  directional.

## Acceptance {#NARROWPLAN-ACCEPTANCE}

- Inference and narrowing have one shared implementation rather than diverging
  rule-local approximations.
- Hover/inlay results and checker diagnostics agree for the same expression.
- The gradual-guarantee differential suite (strip annotations → assert no new
  errors) passes.
- `make test`, mutation/coverage ratchets, benchmarks for touched hot paths, and
  the live 141/141 conformance gate all pass with zero false positives.

## Checklist {#NARROWPLAN-CHECKLIST}

### Stage 0 — bidirectional + constraint foundations

- [ ] Add the two-mode bidirectional core: every AST expression node supports
  `synth(e) → τ` and `check(e, τ)`, with `check` as the primary driver
  ([TYPEINF-TARGET-BIDIRECTIONAL](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-BIDIRECTIONAL)).
- [ ] Thread expected types through container literals, comprehensions, lambda
  parameters, and call arguments; verify Salsa dependency growth stays
  acceptable, else fall back to peek-ahead for the failing constructs only.
- [ ] Add the two-stage constraint architecture: a constraint-generation pass
  producing subtype constraints (`τ₁ <: τ₂`) and a separate solver
  ([TYPEINF-TARGET-CONSTRAINTS](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-CONSTRAINTS)).
- [ ] Represent type variables with explicit lower/upper bounds and
  input/output polarity; defer generalization to first constraining use
  (`list[Var{lower=Literal[1]}]`, not eager `Literal[1] → int`).
- [ ] Build the gradual-guarantee differential test harness: strip annotations
  from a corpus and assert no new errors appear.
- [ ] Prototype-validate the borrowed algebraic-subtyping ideas (polar types,
  constraint simplification) against real typeshed stubs before relying on
  them.

### Stage 1 — incrementality

- [ ] Move inference onto definition-level and expression-level Salsa tracked
  queries (not file-level).
- [ ] Compute a compact per-module interface/signature query as the cross-file
  dependency boundary for early cutoff.
- [ ] Model inference cycles with fixpoint iteration seeded by a
  divergent/bottom sentinel and a hard iteration cap.
- [ ] Measure memory on a 1M+ LOC target; if it blows up, add AST/binding
  eviction behind the query layer (keep only interfaces).
- [ ] Measure p50/p99 keystroke re-check latency on a 1M-LOC corpus; target
  single-digit milliseconds.

### Stage 2 — flow analysis and narrowing

- [ ] Add a scoped narrowing environment with branch push, complement, join,
  and nested-function boundaries.
- [ ] Consume resolver guards for `isinstance`, `is None`, truthiness,
  `TypeGuard`, `TypeIs`, `assert`, and pattern matching.
- [ ] Model assignment narrowing without changing the declared type used for
  assignment validation.
- [ ] Test positive/complement branches, loops, early exits, closures, and
  unreachable branches through public checker behavior.
- [ ] Reformulate narrowing as intersection-and-negation types over a
  Salsa-backed use-def map with `phi`/join at control-flow merges.
- [ ] Extend guard support: `issubclass`, `==`/`in` against literals,
  `hasattr` (synthetic protocol intersection), exhaustiveness/implied-else,
  and TypedDict `"key" in td` narrowing.
- [ ] Decide and document the attribute-narrowing-across-calls tradeoff;
  default to the usable behavior, make it configurable.
- [ ] Replace pattern-matched reachability idioms with inference-driven
  reachability.
- [ ] Measure narrowing richness against the utahplt/ifT-benchmark
  (<https://github.com/utahplt/ift-benchmark>).

### Stage 2 — expression inference

- [ ] Infer same-module and imported function/method return types.
- [ ] Infer constructor, attribute, subscript, binary/unary, conditional, and
  walrus expressions from structured AST data.
- [ ] Centralize builtin constructor/method signatures instead of adding
  rule-local string tables.
- [ ] Reuse the same inference results for diagnostics, hover, completions, and
  inlay hints.
- [ ] Infer unannotated parameter types from body constraints and call sites
  so `BSK-0001` becomes unnecessary where types are recoverable (issue
  [#317](https://github.com/MelbourneDeveloper/Basilisk/issues/317)).
- [ ] Build the curated container/comprehension/lambda benchmark and the
  targeted higher-order (`map`/`filter`/decorators/`ParamSpec`) benchmark.

### Stage 2 — generic constraints

- [ ] Collect lower, upper, constrained, default, and expected-return bounds for
  TypeVars.
- [ ] Solve bounds deterministically and report ambiguity without guessing.
- [ ] Cover constrained/bound TypeVars, PEP 696 defaults, ParamSpec, and
  TypeVarTuple interactions before wiring the solver into rule decisions.

### Stage 2 — shared subtyping

- [ ] Build a context for nominal class relationships, Protocol members,
  TypedDict schemas, generic variance, and Callable parameter kinds.
- [ ] Replace duplicated rule-local subtype helpers only after parity tests pin
  their current accepted/rejected cases.
- [ ] Keep `Any`/`Unknown` gradual behavior and the numeric tower consistent
  across annotation parsing and inferred types.

### Stage 3 — type-level evaluation groundwork

- [ ] Build the normalization-by-evaluation engine for type-level functions as
  memoized Salsa queries returning whnf types.
- [ ] Enforce fuel/depth bounds and memoization of normalized results.
- [ ] Add the `Divergent`/`@Todo` fallback preserving the gradual guarantee on
  truncated evaluation.
- [ ] Add GHC-style (Paterson/Coverage-analogue) acceptance conditions with an
  opt-in "undecidable" escape hatch.
- [ ] Represent mapped types as kind `Type → Type` operators and conditional
  types as guarded rewrites on assignability, evaluated lazily
  (call-by-need).

### Integration and acceptance

- [ ] Introduce each shared component behind existing checker APIs; do not
  create an alternate checking mode.
- [ ] Migrate assignment, return, call, and `assert_type` rules incrementally,
  deleting the replaced local logic in the same change.
- [ ] Add spec-ID-linked mutation-resistant tests for each migrated behavior.
- [ ] Verify hover/inlay results and checker diagnostics agree for the same
  expression.
- [ ] `make test`, mutation/coverage ratchets, benchmarks for touched hot
  paths, and the live 141/141 conformance gate all pass with zero false
  positives.
