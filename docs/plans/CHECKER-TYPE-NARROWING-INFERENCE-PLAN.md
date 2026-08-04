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
- **Winning is the exit criterion, not an aspiration — and integration comes
  first.** Basilisk MUST end this plan with measurably better type inference
  than pyright, mypy, ty, pyrefly, and zuban, wired into the shipped checker —
  a lead held by a detached engine counts for nothing. The plan is not
  complete while any competitor leads any axis in
  [NARROWPLAN-TARGETS](#NARROWPLAN-TARGETS); the mechanism that makes the
  claim honest, enforceable, and permanent is the post-integration scoreboard
  ratchet in [NARROWPLAN-SCOREBOARD](#NARROWPLAN-SCOREBOARD), which starts
  only after [NARROWPLAN-INTEGRATION](#NARROWPLAN-INTEGRATION) has the engine
  behind live diagnostics.

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

## Annotation name resolution {#NARROWPLAN-ANNOTATION-RESOLUTION}

Spec: [TYPEINF-ANNOTATION-RESOLUTION](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION).

**This is the highest-value gap in the plan and a prerequisite for the rest of
it.** The bidirectional engine and constraint solver landed in Stage 0, but the
conformance rules still obtain a declared type by parsing annotation *source
text*: `returns_compatibility.rs` calls
`InferredType::from_annotation(slice_span(&module.source, ann_span))`, and any
name it does not recognise becomes `InferredType::Named(..)`. `shared.rs`'s
`is_unverifiable_return_type` then treats **every** `Named` as unverifiable and
suppresses the diagnostic, recursing through unions, containers, optionals,
tuples, and callables so that one nominal name anywhere in an annotation
silences the whole check.

The consequence is that `-> MyAlias` and `-> MyClass` disable the return check
that `-> int` performs correctly — for PEP 695 aliases, `TypeAlias`, implicit
aliases, same-file classes, and imported classes alike
([#378](https://github.com/Nimblesite/Basilisk/issues/378)). No amount of solver
sophistication helps while the front door takes text.

Work:

- Build the resolution cascade specified in
  [TYPEINF-ANNOTATION-RESOLUTION](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION)
  (alias table → class table → import table → typeshed → forward reference) as
  one shared entry point that every rule and the `bidir` engine consume.
- Expand alias chains transparently, with cycle detection that terminates on
  legal recursive aliases rather than rejecting them
  ([#371](https://github.com/Nimblesite/Basilisk/issues/371)).
- Retire `is_unverifiable_return_type`'s blanket `Named` skip in favour of
  "resolved → check it, unresolved → `Unknown` → suppress". The skip is a
  false-positive guard today; it may only be removed as the cascade makes each
  category genuinely resolvable, one category at a time, with the conformance
  false-positive ceiling held at zero throughout.
- Resolve **decorators** through the same binding table so a decorator reached
  via a local alias (`o = overload`) is recognised as the symbol it is bound to
  ([#380](https://github.com/Nimblesite/Basilisk/issues/380)).

Sequencing: aliases and same-file classes first (pure `ResolvedModule` data,
no new dependencies), then imported project symbols, then typeshed — the last
of which is gated on [#324](https://github.com/Nimblesite/Basilisk/issues/324)
and must not block the first two.

## Call-site and expression coverage {#NARROWPLAN-CALLSITES}

Rules that fire on a call must fire wherever the call *appears*, not only where
it is convenient to find. Two confirmed position-sensitivity defects:

- `constructors_call_init` only fires when the constructor call is the
  outermost expression of a statement or an assignment RHS: `C(1)` is caught,
  `C(1).method()` is not
  ([#381](https://github.com/Nimblesite/Basilisk/issues/381)). Chained
  construction is a mainstream idiom, so this is a large hole.
- A function assigned into a class body is not bound as a method, so the
  implicit receiver is never counted against the declared parameters
  ([#382](https://github.com/Nimblesite/Basilisk/issues/382)).

Both want the same remedy the expression-inference work already implies: a
single traversal that visits every `Call`/`Attribute` node with a resolved
callee type, replacing per-rule statement-shape matching.

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
guessing. Constrained/bound TypeVars, PEP 696 defaults, ParamSpec, and
TypeVarTuple interactions are covered by the solver's pinning tests — the
solver reaches rule decisions through the demolition order in
[NARROWPLAN-INTEGRATION](#NARROWPLAN-INTEGRATION), and any interaction found
uncovered on the way is a test to add, never a reason to stall the wiring.

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

`SubtypingContext` is the **only** subtyping judgment: nominal class
relationships, Protocol members, TypedDict schemas, generic variance, Callable
parameter kinds. The parity tables that once gated the rule-local helpers'
replacement are pinned (`tests/subtyping_context_tests.rs`) — the gate is
**satisfied and closed**. Every remaining rule-local subtype helper is
condemned ([TYPEINF-LEGACY](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-LEGACY));
delete each by routing its callers through `SubtypingContext`, per the
demolition order in [NARROWPLAN-INTEGRATION](#NARROWPLAN-INTEGRATION).
`Any`/`Unknown` gradual behavior and the numeric tower have one home each —
never a per-rule copy.

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

### The mandate

**The bidirectional engine is the checker's type oracle. Full stop.**

There is exactly one component in this repository permitted to decide what
type an expression has: `bidir::BidirEngine`, driven through
`narrow::analyse_function_in` for flow-sensitive positions. Every rule that
needs a type asks it. No rule computes a type any other way. No rule keeps a
private opinion about a type "just for its case". No rule guesses from
punctuation.

Every mechanism that currently decides a type by looking at *source text* or
at *syntactic shape* is legacy. Legacy code is not maintained here, not
tolerated here, and not migrated around — it is **deleted**, in the same
change that replaces it, by the engineer doing the replacing. A change that
routes a rule through the engine while leaving the old path breathing next to
it is **not done and must not merge**.

### The demolition list

Measured on this checkout — reproduce with `grep -rln <pattern>
crates/basilisk-checker/src/rules | wc -l`:

| Legacy mechanism | Rule files | Verdict |
| --- | --- | --- |
| `slice_span` — cutting the annotation out of the source as a **string** | 86 | DELETE |
| `RhsKind` — branching on the syntactic *shape* of a right-hand side | 26 | DELETE |
| `InferredType::from_annotation` over source text — a type parser that is not the parser | 14 | DELETE |
| `rules/shared/text_scan.rs` — hand-rolled character scanning (151 LOC) | shared | DELETE |
| Direct `name_subtype`/`is_numeric_subtype` calls bypassing `subtyping::SubtypingContext` | 22 call sites in 12 files | DELETE |

Out of 172 rule modules. That is the floorboard count. Every one of those call
sites is a rule that today answers a type question by reading characters
instead of asking the engine, and every one of them is a place a real program
gets checked wrong. `assignment_compatibility` is the flagship: it fires on
literal right-hand sides and stays **silent on every call right-hand side**,
which is why `a: int = returns_str()` passes today (Refs #397).

This also finishes [LINESCANPLAN-ELIMINATION](CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md#LINESCANPLAN-ELIMINATION)
by removing the *reason* line scanning exists, not just its call sites.

### There is no obstacle — stop pretending there is

Every piece needed to do this is already built, already tested, and already
reachable from inside a `Rule::check`:

- `rules::shared::parse_module(module)` (`rules/shared.rs:52`) hands any rule
  the module's AST, parsed once and shared through `ResolvedModule::lazy_ast`.
- `narrow::analyse_function_in` returns flow-narrowed types and
  inference-driven unreachability for a function body.
- `BidirEngine::synth` / `check` type any expression bidirectionally, and
  `synth_call` already resolves call returns — the exact thing
  `assignment_compatibility` fails to do.
- `bidir::generics::GenericEnv` and `subtyping::SubtypingContext` are built,
  pinned by tests, and waiting.

Nothing is missing. The only thing that ever held this back was the staging
discipline written in this very section, and the cost defect that discipline
existed to protect against — which is now fixed and measured below. **The
protection has expired. Wire it in.**

`GenericEnv` and `SubtypingContext` leave limbo by being **wired up**, never by
being deleted and never by an `#[allow]` — each stays `pub` from the crate
root, which is what keeps the workspace's `dead_code = "deny"` satisfied.

### What is NOT on the demolition list — read this before touching anything

Ripping out legacy *mechanism* is mandatory. Weakening the *checker* is
forbidden, and nothing in this section licenses it:

- **Never delete, disable, or unregister a rule.** Not one. The rule survives;
  its guts get replaced. See [CHKARCH-CONFORMANCE].
- **Never remove a diagnostic.** Post-migration output is identical or
  strictly better — same code, same span, same or clearer message.
- **Never touch the scoreboard.** 100% / 0 false positives against a freshly
  cloned `python/typing` harness is the prime directive and outranks this
  entire plan. A migration that drops the number is reverted, not negotiated.
- **Never add an alternate checking mode**, feature flag, or "new engine"
  toggle. There is one code path. Basilisk has no modes.

If replacing a rule's guts costs a required error, the engine is not ready for
that rule yet — **fix the engine**, then come back. Do not ship the loss.

### Order of demolition — every step closes filed bugs

This is not speculative refactoring: **each step fixes real, currently open
issues** (`docs/open_issues.csv`). The demolition list IS the bug list. Each
step is one change: wire the rule to the engine, delete the legacy path it
replaces, land the spec-ID-linked mutation-resistant tests, re-certify. The
sequenced checkboxes live in the checklist
([Integration and acceptance](#NARROWPLAN-CHECKLIST)):

1. `assignment_compatibility` → engine (`synth_call` for call RHS), `RhsKind`
   dies — fixes #397 (unfixed half) and the assignment half of #378.
2. `returns_compatibility` / `returns_compatibility_2` → engine synthesis —
   fixes the return half of #378; companion rule `returns_implicit_none`
   (#401) lands in the same family.
3. `calls_argument_type` → engine + `SubtypingContext` — fixes #356 (wrong in
   both directions on `str.join`).
4. One engine-driven traversal visiting **every** `Call` node, not just
   outermost positions ([NARROWPLAN-CALLSITES](#NARROWPLAN-CALLSITES)) —
   fixes #381, #382, and the position half of #335.
5. `directives_assert_type` / `directives_reveal_type` = the hover oracle,
   byte for byte — fixes #290 (solved generics surface everywhere).
6. `BSK-0001` consults `param_infer` before demanding an inferable
   annotation — fixes #317.
7. Text-matching long tail: `slice_span` ~80 consumers → 0 — fixes #379 and
   retires the mechanism behind #383.
8. Flow-analysis dividend: `names_unbound` migrates to the walker's all-paths
   divergence — fixes #285.

### Gates that stay armed the entire time

Non-negotiable, every step, no exceptions:

- Live conformance run: **100% / 0 FP**, freshly cloned harness
  ([CHKARCH-CONFORMANCE-MODE]).
- `make bench`: no fixture slower than the committed baseline
  ([CHKARCH-TESTING-BENCH-RATCHET]). The walker is real production cost the
  moment step 1 lands — record the baseline **in that same change**.
- `make test` fail-fast, coverage ratchet up, mutation ratchet up.
- Torture golden gate green.

### The cost defect that blocked all of this — FIXED

**The flow walker's synthesis path stays UNTIMED until it is wired, so it was
made cheap BEFORE the first rule consumed it — DONE.** The same staging that
kept these cores off live diagnostics also kept them off every performance
gate: `narrow::analyse_function_in` is reached only through the
`narrowed_uses` Salsa query, whose sole callers today are tests and
`examples/`. `make bench` times `basilisk check`, which never enters this code
— so no ratchet was watching it, and a cost that small fixtures hide would
have landed as a *regression on the first wiring change*, when the
zero-tolerance benchmark gate ([CHKARCH-TESTING-BENCH-RATCHET](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING-BENCH-RATCHET))
is suddenly live over it and the change is also carrying diagnostic risk.

The cost was in `FlowWalker::synth_type` (`narrow/flow.rs`), called per
assign/ann-assign RHS, per `for` iterable, per bare-expression statement and
per `while` test. Every call rebuilt a fresh `HashMap<String, Ty>` from **the
entire module's** `ctx.callables` (production seeds this from
`callable_interface` for the whole file), extended it with
`NarrowEnv::visible()`, constructed a fresh `BidirEngine`, and threw all
solver state away via `finish()` — so per-expression work scaled with module
size and the total scaled as roughly function-size × module-size. Divergence
compounded it: `walk_if` probed `body_diverges(&node.body)` and then walked
that same body, whose `walk_stmts` re-probed each statement, re-synthesizing
the same expressions once per enclosing branch.

All three fixes have landed:

- `ctx.callables` converts to `Ty` **once**, in `analyse_function_in`, and
  stays in the engine's outermost scope for the whole walk.
- The walker holds **one** `BidirEngine`. Each expression pushes only the
  visible flow bindings (function-sized, not module-sized) with
  `BidirEngine::push_scope_with`, then resets the solver in place with
  `BidirEngine::solve_expression` rather than dropping the engine. The reset
  is what keeps each expression's solve independent of its predecessors —
  pinned by `bidir::tests::reused_engine_matches_a_fresh_engine_per_expression`,
  which asserts a reused engine answers identically to a fresh one.
- `FlowWalker::one_diverges` memoizes by statement span, so the probe and the
  walk that follows it cannot re-synthesize. Keying on span alone is sound
  because the only synthesis-dependent divergence forms are a call statement
  typed `Never` and a `while` test proven to be a truthy literal, neither of
  which a narrowing frame can change.

The harness that made the cost visible is committed as
`crates/basilisk-checker/examples/narrow_walk_cost.rs`, so the curve is
reproducible rather than asserted:

```sh
cargo run --release -p basilisk-checker --example narrow_walk_cost
```

Self-measured (Apple silicon macOS, `--release`), one walk of a 60-branch
function against a synthetic module of N callables it never mentions,
averaged over 20 walks. The point is the *shape* of the curve, not the
absolute times, which are machine-specific:

| module callables | before | after |
| --- | --- | --- |
| 0 | 581 µs | 470 µs |
| 100 | 957 µs | 462 µs |
| 1 000 | 3.96 ms | 484 µs |
| 5 000 | 18.02 ms | 606 µs |

Cost was linear in module size and is now effectively flat; the residual
growth is the single construction-time conversion of the callable seed,
amortized over the whole walk. The harness also prints `narrowed_uses`, which
must stay at 179 — a "faster" walk that stopped narrowing is a regression, not
a win. Fixing this while the component still had no consumers was strictly
cheaper: no caller to break, no diagnostic to hold steady, no conformance run
to re-certify.

## Measurable targets {#NARROWPLAN-TARGETS}

The axes on which the inference lead is defined and measured. Each axis has
a concrete metric so the lead is provable, not asserted:

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

## Inference scoreboard ratchet {#NARROWPLAN-SCOREBOARD}

**Sequencing: this section is POST-INTEGRATION.** A lead measured on a
detached engine is a lead on nothing — until
[NARROWPLAN-INTEGRATION](#NARROWPLAN-INTEGRATION) has the engine answering
real diagnostics in the shipped binary, there is no product to score, and no
scoreboard work outranks a single demolition step. The torture corpus already
seeded (below, first checklist item) stays live because it scores the *shipped
checker*; the remaining axes are built only after the wiring they would
measure exists.

Basilisk MUST end this plan with better type inference than every
officially-recognized competitor. "Better" is defined operationally and
enforced exactly the way this repo already enforces conformance and speed —
self-measured, reproducible, write-always, ratcheted:

- **Definition.** Basilisk is superior on an axis when it scores strictly
  better than the LATEST official release of every officially-recognized
  competitor (pyright, mypy, ty, pyrefly, zuban — the same set as the speed
  benchmarks) on that axis's metric, measured by Basilisk's own harness with
  the methodology stated in the results. Consistent with the documentation
  honesty rules, we never claim a lead by comparing our numbers against
  vendor-published figures — only same-harness, same-corpus, same-machine
  measurements count.
- **Inference scoreboard harness.** Mirror the `benchmarks/` design: every run
  pulls the latest official release of each competitor and runs the full
  corpus set against all checkers — the reveal_type-precision corpus
  (containers/comprehensions/lambdas/literal-generic precision), the
  utahplt/ifT narrowing benchmark, the higher-order corpus, the
  gradual-guarantee differential suite, and the incremental-latency
  measurement. Scores are written to a status file **immediately and
  unconditionally** (WRITE-ALWAYS); a separate read-only gate compares against
  the committed baseline (GATE-SEPARATELY). A run that measured a score but
  didn't record it is a lie.
- **Ratchet.** Once Basilisk takes the lead on an axis, the lead becomes a CI
  gate: falling behind any competitor on a led axis is a build failure. Leads
  only accumulate. The plan exits only when Basilisk leads **all five axes
  simultaneously** while the 100%/0-FP conformance gate and the speed
  benchmark gate stay green — the inference lead must never be bought by
  regressing conformance or performance, and vice versa.
- **Moving targets.** Because the harness pulls latest competitor releases,
  the lead is continuously re-proven against competitors as they improve —
  never against frozen versions. If a competitor release takes back an axis,
  CI goes red and reclaiming that axis becomes the top-priority work item on
  this plan.
- **Claims discipline.** No "better inference than X" statement ships in
  docs, website, or marketing unless the current committed scoreboard run
  shows the lead, and the claim links to how it is measured.

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
  directional. The only numbers Basilisk acts on are the ones its own
  scoreboard harness produces ([NARROWPLAN-SCOREBOARD](#NARROWPLAN-SCOREBOARD)).
- **Competitors are moving targets.** Pyrefly and ty ship fast and are well
  funded; ty is actively closing its bidirectional gap. The scoreboard ratchet
  is designed for this: leads are re-proven against latest releases on every
  run, and a lost axis turns CI red rather than silently eroding the claim.

## Acceptance {#NARROWPLAN-ACCEPTANCE}

- Inference and narrowing have one shared implementation rather than diverging
  rule-local approximations.
- Hover/inlay results and checker diagnostics agree for the same expression.
- The gradual-guarantee differential suite (strip annotations → assert no new
  errors) passes.
- The inference scoreboard ([NARROWPLAN-SCOREBOARD](#NARROWPLAN-SCOREBOARD))
  shows Basilisk strictly ahead of the latest official releases of pyright,
  mypy, ty, pyrefly, and zuban on **every** axis in
  [NARROWPLAN-TARGETS](#NARROWPLAN-TARGETS), and the per-axis ratchet is wired
  into CI so the lead cannot silently erode.
- `make test`, mutation/coverage ratchets, benchmarks for touched hot paths, and
  the live 141/141 conformance gate all pass with zero false positives.

## Checklist {#NARROWPLAN-CHECKLIST}

### Stage 0 — bidirectional + constraint foundations

- [x] Add the two-mode bidirectional core: every AST expression node supports
  `synth(e) → τ` and `check(e, τ)`, with `check` as the primary driver
  ([TYPEINF-TARGET-BIDIRECTIONAL](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-BIDIRECTIONAL)).
  — `crates/basilisk-checker/src/bidir/{engine,check}.rs`.
- [x] Thread expected types through container literals, comprehensions, lambda
  parameters, and call arguments; verify Salsa dependency growth stays
  acceptable, else fall back to peek-ahead for the failing constructs only.
  — Verified by architecture: the engine is a pure function of one module's
  AST inside the existing file-level tracked queries, so it adds zero Salsa
  edges (see the Salsa note in `crates/basilisk-checker/src/bidir/mod.rs`);
  re-evaluate per construct when Stage 1 moves to finer-grained queries.
- [x] Add the two-stage constraint architecture: a constraint-generation pass
  producing subtype constraints (`τ₁ <: τ₂`) and a separate solver
  ([TYPEINF-TARGET-CONSTRAINTS](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-CONSTRAINTS)).
  — `crates/basilisk-checker/src/bidir/{constraints,solve}.rs`; every ground
  leaf delegates to `InferredType::is_assignable_to`.
- [x] Represent type variables with explicit lower/upper bounds and
  input/output polarity; defer generalization to first constraining use
  (`list[Var{lower=Literal[1]}]`, not eager `Literal[1] → int`).
  — `crates/basilisk-checker/src/bidir/tyvar.rs`; the exact
  `list[Var{lower=Literal[1]}]` case is a unit test.
- [x] Build the gradual-guarantee differential test harness: strip annotations
  from a corpus and assert no new errors appear.
  — `crates/basilisk-checker/tests/gradual_guarantee_tests.rs` (curated
  corpus + a sweep of all synced conformance fixtures); it immediately caught
  and drove the fix of a real rule defect (`classes_override` treated an
  absent annotation as a signature mismatch).
- [x] Prototype-validate the borrowed algebraic-subtyping ideas (polar types,
  constraint simplification) against real typeshed stubs before relying on
  them.
  — `crates/basilisk-checker/tests/bidir_typeshed_validation_tests.rs` over
  five verbatim `python/typeshed` stubs (commit pinned in
  `tests/fixtures/typeshed/TYPESHED_COMMIT.txt`): solver reflexivity,
  projection idempotence, and polar-variable resolution over 300+ real
  annotations.

### Stage 0.5 — annotation name resolution

Prerequisite for Stage 2; see
[NARROWPLAN-ANNOTATION-RESOLUTION](#NARROWPLAN-ANNOTATION-RESOLUTION). Each box
lands with a regression test that fails before it and passes after, and holds
the conformance ratchets (100% / 0 false positives) at every step.

- [ ] Add one shared `resolve_annotation(module, expr) → InferredType` entry
  point implementing the
  [TYPEINF-ANNOTATION-RESOLUTION](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION)
  cascade over the Ruff AST annotation node, replacing
  `InferredType::from_annotation(<source text>)`. No rule may parse annotation
  text after this lands.
- [ ] Resolve PEP 695 `type` aliases, `X: TypeAlias = ...`, and implicit
  aliases, including alias chains and use-before-declaration; expand
  transparently at every nesting depth.
- [ ] Resolve same-file classes, then imported project symbols; leave typeshed
  behind the same entry point so [#324](https://github.com/Nimblesite/Basilisk/issues/324)
  can fill it without a second call path.
- [ ] Replace the blanket `Named` skip in `rules/shared.rs::is_unverifiable_return_type`
  with a resolved/unresolved split, narrowing it one category at a time as the
  cascade covers that category.
- [x] Terminating cycle detection for recursive aliases: `type J = list[J]`,
  `type J = int | list[J]`, `type J = dict[str, J]`, and the canonical
  `JsonValue` union all produce **no** diagnostic
  ([#371](https://github.com/Nimblesite/Basilisk/issues/371)).
  — Delivered by the Stage 3 acceptance conditions: `tyeval::accept::classify`
  admits guarded recursion (constructor subscripts guard; union arms alone do
  not), and `generics_syntax_scoping::check_type_alias_circular` reports only
  `Unguarded`/`NonRegular` verdicts. All four #371 forms (plus a JsonValue
  arm-order permutation) are pinned clean in
  `tests/checker/generics_syntax_scoping_tests.rs`.
- [x] Add PEP 695 `type`-statement counterparts of every recursive case in
  upstream `aliases_recursive.py` to our own suite — the upstream file contains
  zero `type` statements, which is why this false positive survived a 100%
  score. Coverage of a syntax the upstream suite omits is our responsibility.
  — `tests/checker/aliases_recursive_tests.rs`: every recursive alias
  DEFINITION (`Json`/`Json2`, `RecursiveTuple`, `RecursiveMapping`, both
  generic aliases + specialization) pinned clean as a `type` statement, and
  both `# E: cyclical reference` cases (`RecursiveUnion` in `|` and
  `Union[..]` spellings, the `MutualReference` pair) pinned firing.
  Value-level assignability THROUGH these aliases is the annotation-resolution
  cascade's box above, not this one.
- [ ] Resolve decorator expressions through the binding table so `o = overload`
  is recognised as `typing.overload`; cover `from typing import overload as ov`
  and `typing.overload` / `t.overload` attribute spellings
  ([#380](https://github.com/Nimblesite/Basilisk/issues/380)).
- [ ] Visit calls in every expression position rather than statement-outermost
  only, so `C(1).method()` reports the same constructor-arity error as `C(1)`
  ([#381](https://github.com/Nimblesite/Basilisk/issues/381)).
- [ ] Bind functions assigned in a class body as methods — implicit receiver
  consumed on instance access, unbound on class access, `staticmethod` /
  `classmethod` honoured ([#382](https://github.com/Nimblesite/Basilisk/issues/382)).
- [ ] Wire the shared entry point into the `bidir` engine, which currently has
  no name resolution at all and is consumed by only two rules
  (`narrowing_typeguard`, `narrowing_typeis_2`).

### Stage 1 — incrementality

- [x] Move inference onto definition-level and expression-level Salsa tracked
  queries (not file-level).
  — `crates/basilisk-checker/src/incremental_defs.rs`: a tracked
  `Definition` struct per top-level definition (keyed on the definition's own
  source *slice*, so edits elsewhere leave its memos untouched), with
  `definition_type` (per-definition) and `expression_types`
  (per-expression) queries. Early cutoff is proven by salsa's `WillExecute`
  log in `tests/incremental_defs_tests.rs`: editing one definition
  re-executes exactly one `definition_type`.
- [x] Compute a compact per-module interface/signature query as the cross-file
  dependency boundary for early cutoff.
  — `module_interface` returns a `PartialEq` `(name, type)` list; a
  body-only edit backdates to "unchanged" (test:
  `body_only_edit_backdates_the_module_interface`).
- [x] Model inference cycles with fixpoint iteration seeded by a
  divergent/bottom sentinel and a hard iteration cap.
  — `definition_type` opts into salsa fixpoint iteration
  (`cycle_initial` = `Unknown`, the divergent/bottom sentinel;
  `cycle_fn` caps at `CYCLE_ITERATION_CAP = 16` and falls back to the
  sentinel). `a = b; b = a` terminates and settles on `Unknown` (test).
- [x] Measure memory on a 1M+ LOC target; if it blows up, add AST/binding
  eviction behind the query layer (keep only interfaces).
  — Self-measured via `examples/incremental_measure.rs` over the seeded
  synthetic corpus from `scripts/gen_incremental_corpus.py` (2,100 files,
  1,117,199 LOC, 268,800 definitions): RSS ≈ 312 MB after the cold pass and
  a 200-edit keystroke loop — no blow-up, so eviction is not needed at this
  stage (re-measure when the per-definition queries carry richer state).
- [x] Measure p50/p99 keystroke re-check latency on a 1M-LOC corpus; target
  single-digit milliseconds.
  — Same harness, same corpus: **p50 0.16 ms, p99 0.22 ms** per keystroke
  re-check of the edited file's definition-level queries (cold pass 0.99 s).
  Scope caveat, stated plainly: this measures the Stage 1 definition-level
  query layer, not the full 165-rule diagnostics pipeline, which remains
  file-level until the Integration stage migrates it.

### Stage 2 — flow analysis and narrowing

- [x] Add a scoped narrowing environment with branch push, complement, join,
  and nested-function boundaries.
  — `crates/basilisk-checker/src/narrow/env.rs` (`NarrowEnv`: branch frames,
  a whole-scope layer for `assert`/early-exit facts, `phi`-join at merges,
  fresh-environment nested-function boundary) plus the statement-level
  walker in `narrow/flow.rs` (early-exit complement persistence, loop and
  try/except frames discarded).
- [x] Consume resolver guards for `isinstance`, `is None`, truthiness,
  `TypeGuard`, `TypeIs`, `assert`, and pattern matching.
  — `narrow/guards.rs` (two-branch outcomes with PEP 647/742 asymmetry,
  loop-guard suppression) + `narrow/flow.rs` (span-matched application,
  `match` per-case subject narrowing with value-pattern conservatism).
- [x] Model assignment narrowing without changing the declared type used for
  assignment validation.
  — `NarrowEnv` keeps the declared layer immutable (`declared()` is the
  validation anchor); `x = expr` narrows only the flow layer via the
  bidirectional engine's synthesized type (tests in
  `tests/narrow_flow_tests.rs` and `narrow/env.rs`).
- [x] Test positive/complement branches, loops, early exits, closures, and
  unreachable branches through public checker behavior.
  — `tests/narrow_flow_tests.rs` runs the real parse → resolve →
  `analyse_function` pipeline over all six cases (unreachable branches via
  the `Never` narrowing signal). Diagnostic-level surfacing of these
  results lands with the Integration-stage rule migration.
- [x] Reformulate narrowing as intersection-and-negation types over a
  Salsa-backed use-def map with `phi`/join at control-flow merges.
  — Intersection/negation: `narrow/set_ops.rs` (`intersect`/`subtract`
  delegating atoms to `is_assignable_to`); `phi`/join: `NarrowEnv::join`;
  Salsa backing: the tracked `narrowed_uses` query in
  `incremental_defs.rs` keys the whole flow analysis on one definition's
  source slice — editing one function re-executes only its own narrowing
  (proven by `WillExecute` log in `tests/incremental_defs_tests.rs`).
- [x] Extend guard support: `issubclass`, `==`/`in` against literals,
  `hasattr` (synthetic protocol intersection), exhaustiveness/implied-else,
  and TypedDict `"key" in td` narrowing.
  — Resolver extraction for all five (`visitor/narrowing.rs`, new
  `NarrowingGuardKind` variants with case-preserving literal capture) and
  checker interpretation (`narrow/guards.rs`): `==`/`in` literal
  narrowing with exact-literal complements, `"key" in td` union filtering
  over a `NarrowContext` of `TypedDict` key sets (required vs optional),
  and implied-else after fully-diverging `match` statements. `issubclass`
  and `hasattr` interpretation is deliberately identity until `type[...]`
  object modelling and synthetic-protocol intersections land with the
  shared-subtyping work — extraction is live, so flipping them on is a
  local change.
- [x] Decide and document the attribute-narrowing-across-calls tradeoff;
  default to the usable behavior, make it configurable.
  — Decided and recorded in
  [TYPEINF-NARROWING-ATTR-CALLS](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-ATTR-CALLS):
  narrows survive calls by default (usable), and
  `narrow-attributes-across-calls = false` in `[tool.basilisk]` opts into
  the sound-but-strict behavior
  (`BasiliskConfig::narrow_attributes_across_calls`, parsed + merged in
  `crates/basilisk-config/src/parse.rs`).
- [x] Replace pattern-matched reachability idioms with inference-driven
  reachability.
  — `narrow/reachability.rs`: divergence is decided by ASKING THE ENGINE
  (a `SynthFn` synthesis oracle) — a call statement diverges iff its
  synthesized type is `Never`, never by matching callee names. Compound
  forms recurse (`if`/`else` both diverge, `while True` without `break`,
  fully-diverging `match` with a wildcard arm, `try`/`finally`), and the
  flow walker records everything after a proven-diverging statement as
  unreachable (`narrow/flow.rs::walk_stmts`). Gradual posture: `Unknown`
  never fabricates divergence. Unit tests in `reachability.rs`; pipeline
  tests in `tests/narrow_flow_tests.rs`.
- [x] Measure narrowing richness against the utahplt/ifT-benchmark
  (<https://github.com/utahplt/ift-benchmark>).
  — Harness: `crates/basilisk-checker/examples/ift_measure.rs` over a fresh
  clone's `Pyright/main.py` (benchmark commit `cfb31ef` at measurement
  time). Self-measured baseline, methodology stated in the harness header:
  **11/37 functions (29%) produce a flow-narrowing signal** (the
  `type(x) is C` positive/negative/`@final` families). The silent families
  each map to a known pending guard form — boolean connectives in tests,
  nested conditions, cross-function `TypeGuard` calls, attribute
  narrowing ([TYPEINF-NARROWING-ATTR-CALLS]), and tuple element/length
  narrowing — so this number is the ratchet floor, not a claim of
  benchmark conformance (that needs the full diagnostics pipeline at the
  Integration stage).

### Stage 2 — expression inference

- [x] Infer same-module and imported function/method return types.
  — Same-module: unannotated returns synthesize from the body
  (`incremental_defs::function_type`) and `x = f()` resolves through the
  sibling's declared-or-synthesized return via the backdated
  `callable_interface` query. Imported: `param_infer::imported_callable_globals`
  maps the cross-module layer's `imported_symbols` (functions →
  `Callable[..., R]` from their return annotation, classes → instance form,
  variables → their annotation) into engine scope. Builtin METHOD returns
  come from the centralized table.
- [x] Infer constructor, attribute, subscript, binary/unary, conditional, and
  walrus expressions from structured AST data.
  — All in the bidirectional engine over the ruff AST: constructors
  (`Named` callee → instance), plain attribute loads via per-module class
  schemas (`class_attribute_interface`, backdated; `Point().x` → `float`),
  subscripts (list/dict/tuple-position/str with literal-index precision),
  the binary/unary tables, conditional unions, and walrus binding.
  `self.attr` assignments in `__init__` extend the schema as a follow-up.
- [x] Centralize builtin constructor/method signatures instead of adding
  rule-local string tables.
  — `crates/basilisk-checker/src/bidir/builtins.rs`: one table for builtin
  call returns and `str`/`list`/`dict`/`set` method returns, consumed by
  the engine's call synthesis; argument-dependent builtins deliberately
  stay `Unknown` rather than guessed. Existing rule-local tables migrate
  onto it at the Integration stage.
- [x] Reuse the same inference results for diagnostics, hover, completions, and
  inlay hints.
  — One entry point: `crates/basilisk-lsp/src/util.rs::rhs_or_expr_type_display`
  answers from the resolver's `RhsKind` table first (stable displays) and
  falls back to the SAME bidirectional engine the checker's
  `expression_types` query uses (`inference::infer_expression_source` +
  `display_widened`, gated by `is_fully_known` so a partial
  `list[Unknown]` renders as silence, [TYPEINF-TARGET-GRADUAL]).
  Consumers: hover variable/attribute signatures and member-access
  receiver resolution (`hover/access.rs::receiver_type_name`), dot
  completions (`hover/members.rs::dot_receiver_builtin_type`; the
  completion handler now enriches its re-resolve via
  `resolve_module_imports`, which also made builtin-receiver dot
  completions live), and inlay hints (`inlay_hints.rs`). E2E: engine-only
  receivers (`name = "a".upper()`) hover, complete, and hint in
  `ws_test_hover.rs` / `ws_test_completion.rs` / `ws_test_inlay_hints.rs`.
  Known limit: function return-type inlay hints still go through
  `infer_return_type_display` (the resolver's `ReturnStmtInfo` carries no
  value span to hand the engine).
- [x] Infer unannotated parameter types from body constraints and call sites
  so `BSK-0001` becomes unnecessary where types are recoverable (issue
  [#317](https://github.com/MelbourneDeveloper/Basilisk/issues/317)).
  — Pure core in `crates/basilisk-checker/src/param_infer.rs`: parameters
  bind to input-polarity variables; BODY constraints (passing `p` to a
  callee with a declared parameter type) accumulate demands, same-module
  CALL SITES accumulate lower bounds, and resolution follows input
  polarity (demand wins, else union of flows, else `Unknown` — never a
  guess). Wiring into the `BSK-0001` exemption happens at the Integration
  stage, where the [TYPEINF-EXCEEDS-REQUIRED] predicate widens in
  lockstep with this inference.
- [x] Build the curated container/comprehension/lambda benchmark and the
  targeted higher-order (`map`/`filter`/decorators/`ParamSpec`) benchmark.
  — `crates/basilisk-checker/tests/inference_corpus_tests.rs`: a ratcheted
  precision corpus over the definition-level queries. Equivalence is
  mutual assignability PLUS a gradual-honesty check (an `Unknown` answer
  never silently matches a concrete expectation — the naive scorer
  inflated 23/24; the honest score is **18/24**, `PRECISION_FLOOR = 18`,
  up-only). The six misses are the documented gaps: nested-union
  normalization, lambda parameter display, and the higher-order family
  (`map`/`filter`/decorators) that the generic-constraints stage closes.

### Stage 2 — generic constraints

- [x] Collect lower, upper, constrained, default, and expected-return bounds for
  TypeVars.
  — `crates/basilisk-checker/src/bidir/generics.rs` (`GenericEnv`): the
  declared-generics layer over the engine's anonymous `TyVarStore`.
  Declarations carry `bound=`, constrained value sets, and PEP 696
  defaults; evidence accumulates as deduplicated lower bounds (argument
  flows), upper bounds (expected-return propagation records the demanded
  type), `ParamSpec` parameter-list captures, and `TypeVarTuple` element
  captures — wrong-kind evidence is flagged, never silently dropped.
- [x] Solve bounds deterministically and report ambiguity without guessing.
  — `GenericEnv::resolve`: the answer depends only on the declaration and
  deduplicated evidence. Joins keep literal precision (deferred
  generalization); a constrained var solves to exactly ONE listed
  constraint; evidence supporting several incomparable answers returns
  `Resolution::Ambiguous` with every candidate, no evidence and no
  default returns `Unsolved`, and contradictions return `Unsatisfiable`
  with both sides — never a guess ([TYPEINF-EXCEEDS-NOUNKNOWN]). Ground
  checks delegate to `is_assignable_to`, so `Any` stays gradual.
- [x] Cover constrained/bound TypeVars, PEP 696 defaults, ParamSpec, and
  TypeVarTuple interactions before wiring the solver into rule decisions.
  — `tests/generic_constraints_tests.rs` (21 tests): bound enforcement,
  constraint selection/widening (`Literal["a"]` → the `str` constraint),
  split-selection ambiguity, upper-narrowed constraint sets, defaults
  used only without evidence (including the gradual `...` `ParamSpec`
  default), capture conflicts, elementwise `TypeVarTuple` joins with
  mixed-length ambiguity, and kind-conflict reporting. Rule wiring is
  Integration-stage by design ([NARROWPLAN-INTEGRATION]).

### Stage 2 — shared subtyping

- [x] Build a context for nominal class relationships, Protocol members,
  TypedDict schemas, generic variance, and Callable parameter kinds.
  — `crates/basilisk-checker/src/subtyping.rs` (`SubtypingContext`):
  cycle-guarded transitive nominal walk, structural Protocol satisfaction
  (inherited members count, missing/incompatible members reject),
  `TypedDict` schemas (required/`NotRequired`, `ReadOnly` covariant vs
  mutable invariant), declared per-position variance
  (invariant-by-default), and `Callable` contravariant-params /
  covariant-return with gradual `...`. Tested in
  `tests/subtyping_context_tests.rs`; rules consume it at the
  Integration stage ([NARROWPLAN-INTEGRATION]).
- [x] Replace duplicated rule-local subtype helpers only after parity tests pin
  their current accepted/rejected cases.
  — The text-level tower now has ONE home (`subtyping::name_subtype`),
  pinned by the parity table in `tests/subtyping_context_tests.rs`; the
  provably-identical helpers delegate to it (`rules/shared.rs::
  is_numeric_subtype`, `narrowing_typeis`, `narrowing_typeis_2`,
  `overloads_evaluation`, `generics_typevartuple_callable`,
  `generics_syntax_scoping/alias_misuse`, `callables_subtyping` keeping
  its local `Any`/`object` acceptances, `aliases_implicit` keeping its
  conservative unknown-bound accept). The two DELIBERATELY-different
  helpers stay local with their behavior pinned in place
  (`rules/generics_basic_3/helper_parity_tests.rs`: bool<:int-only table
  + nominal walk; `rules/protocols_generic/helper_parity_tests.rs`:
  conservative TypeVar heuristic) — they merge into `SubtypingContext`
  at Integration behind those same pins.
- [x] Keep `Any`/`Unknown` gradual behavior and the numeric tower consistent
  across annotation parsing and inferred types.
  — `tests/subtyping_context_tests.rs`: `Any`/`Unknown` bidirectional at
  the `InferredType` layer and `Any`-either-side/`object`-top at the
  context layer; the tower asserted to answer identically at the
  annotation-text and `InferredType` layers wherever both define the
  relation (`complex` stays text-level only — the parser folds it to
  `Float`, the documented [TYPEINF-SUBTYPING-NOMINAL] trade-off).

### Stage 3 — type-level evaluation groundwork

- [x] Build the normalization-by-evaluation engine for type-level functions as
  memoized Salsa queries returning whnf types.
  — `crates/basilisk-checker/src/tyeval/`: PEP 695 `type` statements
  lower from the Ruff AST (string forward refs re-parsed) into
  `TypeTerm`s; `Evaluator::eval_at` normalizes to whnf behind the
  `#[salsa::tracked]` queries `type_alias_env` / `alias_whnf`.
  `tests/tyeval_salsa_tests.rs` proves via the `EventDb` `WillExecute`
  log that an unrelated edit backdates the env and serves the memo (zero
  re-executions) while an alias edit re-normalizes exactly once.
- [x] Enforce fuel/depth bounds and memoization of normalized results.
  — `eval.rs`: `EVAL_FUEL = 256`, `EVAL_DEPTH = 64`, per-`(alias, args)`
  memo under the Salsa layer; `tyeval_public_api_tests.rs` pins that
  mutually recursive `Left`/`Right` truncate instead of hanging.
- [x] Add the `Divergent`/`@Todo` fallback preserving the gradual guarantee on
  truncated evaluation.
  — `Eval::Divergent` projects to `InferredType::Unknown`
  ([TYPEINF-TARGET-GRADUAL]): exhausted fuel/depth, ill-kinded
  applications, and `Unknown` conditional scrutinees all truncate
  gradually — never an invented diagnostic.
- [x] Add GHC-style (Paterson/Coverage-analogue) acceptance conditions with an
  opt-in "undecidable" escape hatch.
  — `accept::classify`: `Unguarded` (self-reference not under a type
  constructor; union arms do NOT guard, matching conformance
  `aliases_recursive.py`) and `NonRegular` (growing self-application
  args). `AliasEnv::insert` is acceptance-gated; `insert_undecidable`
  opts out with fuel as the safety net. Wired into
  `generics_syntax_scoping::check_type_alias_circular`, fixing issue
  #371 (pins in `tests/checker/generics_syntax_scoping_tests.rs`).
- [x] Represent mapped types as kind `Type → Type` operators and conditional
  types as guarded rewrites on assignability, evaluated lazily
  (call-by-need).
  — `term.rs`: `Kind::{Type, Operator}`, `TypeTerm::Op`/`Apply` with
  higher-order application through parameters; `TypeTerm::Cond` rewrites
  on `is_assignable_to`, forces only the taken arm, distributes over
  union scrutinees, and stays gradual on `Unknown` scrutinees. Surface
  syntax awaits a ratified PEP 827; the engine is ready behind it.

### Integration and acceptance

- [x] **Blocked every item below.** `FlowWalker::synth_type` is cheap before
  any rule consumes `narrowed_uses` — see [NARROWPLAN-INTEGRATION](#NARROWPLAN-INTEGRATION)
  for the measurements. All three changes landed with the walker still
  unconsumed: (a) `ctx.callables` converts to `Ty` once at walker
  construction; (b) one long-lived `BidirEngine` takes the visible-binding
  overlay through `push_scope_with`/`pop_scope` and resets its solver through
  `solve_expression`; (c) `one_diverges` memoizes by statement span, so the
  `walk_if` probe-then-walk and the `walk_stmts` re-probe no longer
  re-synthesize the same expressions. Cost went from linear in module size to
  flat.
- [ ] Record a `make bench` baseline on a fixture that actually exercises the
  flow walker **in the same change that first wires it**, so the walker stops
  being invisible to the ratchet the moment it starts costing real time.
- [ ] **Step 1 — `assignment_compatibility` to the engine; delete its
  `RhsKind` shape-matching in the same change.** Every right-hand side —
  literal, call, constructor, method, variable — is typed by
  `BidirEngine::synth` (`synth_call` resolves call returns), narrowed by
  `narrow::analyse_function_in`, and judged by `SubtypingContext`.
  `a: int = returns_str()` must fire. Fixes #397 (unfixed half) and the
  assignment half of #378. Not optional.
- [ ] **Step 2 — `returns_compatibility` / `returns_compatibility_2`
  synthesize the returned expression through the engine**, deleting the
  replaced pattern-matching. Fixes the return half of #378. Companion
  default-on rule `returns_implicit_none` (#401) lands in the same family.
- [ ] **Step 3 — `calls_argument_type` judges arguments through the engine +
  `SubtypingContext`**, deleting the syntactic-shape comparison. Fixes #356
  (false positives on valid `str.join` list displays AND a missed genuine
  error).
- [ ] **Step 4 — one engine-driven traversal visits every `Call` node with a
  resolved callee**, not just outermost-expression positions
  ([NARROWPLAN-CALLSITES](#NARROWPLAN-CALLSITES)). Fixes #381, #382, and the
  position half of #335.
- [ ] **Step 5 — `directives_assert_type` / `directives_reveal_type` answer
  from the hover oracle, byte for byte** (the parked
  `directives_assert_type_2` comes alive here). Fixes #290 — solved generic
  parameters surface in hover and diagnostics alike.
- [ ] **Step 6 — `BSK-0001` consults `param_infer` before demanding an
  annotation the engine can already infer** from body constraints and call
  sites. Fixes #317.
- [ ] **Step 7 — text-matching long tail to zero.** Drive
  `grep -rln slice_span crates/basilisk-checker/src/rules | wc -l` from
  **86 to 0**, `RhsKind` (26 files) and `InferredType::from_annotation` over
  source text (14 files) to **0**, and delete `rules/shared/text_scan.rs`
  outright. An annotation is a type expression the engine evaluates — never a
  string a rule slices out of the file. Fixes #379; retires the mechanism
  behind #383.
- [ ] **Step 8 — `names_unbound` migrates to the walker's all-paths
  divergence analysis**, replacing the last-statement idiom. Fixes #285.
- [ ] Route all 22 direct `name_subtype`/`is_numeric_subtype` call sites (12
  files) through `subtyping::SubtypingContext` and delete the shims — runs
  alongside steps 3–7. One subtyping implementation. Not two, not
  twenty-two.
- [ ] Delete every replaced code path **in the change that replaces it**. A
  migration that leaves the legacy path alive alongside the new one is
  incomplete and does not merge.
- [ ] Never create an alternate checking mode, engine flag, or opt-in switch
  for any of this. One code path. Basilisk has no modes
  ([CHKARCH-CONFIGURATION-ONLY]).
- [ ] Keep every rule registered and every diagnostic intact through the whole
  demolition. The mechanism dies; the checking does not. A migration that
  costs a required error means the engine is not ready — **fix the engine**,
  never ship the loss ([CHKARCH-CONFORMANCE]).
- [ ] Add spec-ID-linked mutation-resistant tests for each migrated behavior.
- [ ] Verify hover/inlay results and checker diagnostics agree for the same
  expression — byte for byte, because after this they are the same oracle.
- [ ] `make test`, mutation/coverage ratchets, benchmarks for touched hot
  paths, and the live conformance gate all pass with zero false positives.

### Inference scoreboard ratchet — post-integration

Nothing below (except the already-live torture corpus) starts before the
demolition order above it is complete: score the shipped checker, not a
detached engine.

- [x] Seed the scoreboard with a **type-torture corpus**: hard, spec-grounded
  problems (several straight from the issue tracker) scored conformance-style
  against every competitor, with hang detection as a correctness axis.
  — `benchmarks/torture/`: eight cases (`cases/*.py`, each header citing the
  typing-spec section/PEP that makes its expectations authoritative — #371
  recursive aliases, #398 recursive-base termination, #374 enum literal
  expansion, #317 gradual unannotated code, #284 tuple indexing, PEP 742
  `TypeIs`, PEP 612 `ParamSpec` preservation, generic constructor solving) +
  `run_torture.py` (out-of-the-box defaults for every tool, best-effort
  latest-release pull, per-invocation timeout scored as `hang`, WRITE-ALWAYS
  `status/torture.csv` after every case, read-only regression gate against
  the committed baseline, exit 3). The first measured run (2026-08-04)
  landed basilisk at 5/8, pinning three live defects; all three are fixed —
  the #374 enum-expansion false positive (enum literal expansion
  equivalence, `enum_expand.rs`), the #398 recursive-base hang (iterative
  visited-set base walk, zero recursion), and the module-level fixed-tuple
  index MISS (`visitor/annotated_tuple_index.rs`). Rerun same day, same
  harness (versions in the CSV header): **basilisk 8/8 — tied with mypy and
  pyrefly for the lead; ahead of pyright 7/8, zuban 7/8, ty 4/8.** The
  standing is held twice over: the scoreboard's read-only gate, and
  `crates/basilisk-checker/tests/torture_golden_tests.rs`, which scores all
  eight cases in-process on every `cargo test` so CI breaks the moment a
  case regresses.
- [ ] Build the inference scoreboard harness mirroring `benchmarks/`: pull the
  latest official release of each competitor (pyright, mypy, ty, pyrefly,
  zuban) every run; write scores to a status file immediately and
  unconditionally; gate read-only against the committed baseline.
  (The torture runner above implements the full write-always/gate/pull
  contract for its own corpus; this box widens the same mechanism to the
  five measurable-target axes.)
- [ ] Build the reveal_type-precision corpus
  (containers/comprehensions/lambdas/literal-generic precision) and score all
  checkers on it.
- [ ] Wire the utahplt/ifT narrowing benchmark, the higher-order corpus, the
  gradual-guarantee differential suite, and the incremental-latency
  measurement into the scoreboard.
- [ ] Add per-axis ratchet entries: once Basilisk leads an axis, falling
  behind any competitor on that axis fails CI; leads only accumulate.
- [ ] Take and hold the lead on **all five axes simultaneously**, with the
  100%/0-FP conformance gate and the speed benchmark gate green in the same
  run.
- [ ] Enforce claims discipline: every better-than-competitor claim in docs, website,
  or marketing traces to the current committed scoreboard run and states the
  methodology.
