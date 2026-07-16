# Type Narrowing and Inference Plan {#NARROWPLAN-INFERENCE}

Specs: [TYPEINF-OVERVIEW](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-OVERVIEW)
and [CHKARCH-INFERENCE](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INFERENCE).

Basilisk already passes the typing conformance suite. This plan is therefore
about consolidating duplicated rule-local inference and improving editor/user
behavior without weakening the zero-false-positive gate.

Current foundations include `InferredType`, annotation parsing, literal RHS
inference, resolver-collected narrowing guards, and several rule-local
Protocol/TypedDict/Callable algorithms. There is no shared checker-side flow
environment, expression inferrer, constraint solver, or subtype context.

## Flow analysis {#NARROWPLAN-FLOW}

- [ ] Add a scoped narrowing environment with branch push, complement, join, and
  nested-function boundaries.
- [ ] Consume resolver guards for `isinstance`, `is None`, truthiness,
  `TypeGuard`, `TypeIs`, `assert`, and pattern matching.
- [ ] Model assignment narrowing without changing the declared type used for
  assignment validation.
- [ ] Test positive/complement branches, loops, early exits, closures, and
  unreachable branches through public checker behavior.

## Expression inference {#NARROWPLAN-EXPRESSIONS}

- [ ] Infer same-module and imported function/method return types.
- [ ] Infer constructor, attribute, subscript, binary/unary, conditional, and
  walrus expressions from structured AST data.
- [ ] Centralize builtin constructor/method signatures instead of adding
  rule-local string tables.
- [ ] Reuse the same inference results for diagnostics, hover, completions, and
  inlay hints.

## Generic constraints {#NARROWPLAN-CONSTRAINTS}

- [ ] Collect lower, upper, constrained, default, and expected-return bounds for
  TypeVars.
- [ ] Solve bounds deterministically and report ambiguity without guessing.
- [ ] Cover constrained/bound TypeVars, PEP 696 defaults, ParamSpec, and
  TypeVarTuple interactions before wiring the solver into rule decisions.

## Shared subtyping {#NARROWPLAN-SUBTYPING}

- [ ] Build a context for nominal class relationships, Protocol members,
  TypedDict schemas, generic variance, and Callable parameter kinds.
- [ ] Replace duplicated rule-local subtype helpers only after parity tests pin
  their current accepted/rejected cases.
- [ ] Keep `Any`/`Unknown` gradual behavior and the numeric tower consistent
  across annotation parsing and inferred types.

## Integration {#NARROWPLAN-INTEGRATION}

- [ ] Introduce each shared component behind existing checker APIs; do not create
  an alternate checking mode.
- [ ] Migrate assignment, return, call, and `assert_type` rules incrementally,
  deleting the replaced local logic in the same change.
- [ ] Add spec-ID-linked mutation-resistant tests for each migrated behavior.

## Acceptance {#NARROWPLAN-ACCEPTANCE}

- Inference and narrowing have one shared implementation rather than diverging
  rule-local approximations.
- Hover/inlay results and checker diagnostics agree for the same expression.
- `make test`, mutation/coverage ratchets, benchmarks for touched hot paths, and
  the live 141/141 conformance gate all pass with zero false positives.
