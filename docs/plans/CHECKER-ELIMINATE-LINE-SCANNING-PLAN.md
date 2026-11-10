# Delete Checker Text Matching {#LINESCANPLAN-ELIMINATION}

Checker rules must decide from the Ruff AST and `ResolvedModule` data.
Reconstructing Python structure with `source.lines()` plus `starts_with`, `find`,
or `contains` is parser duplication: it classifies strings and comments as code,
and it makes a diagnostic depend on how the source is spelled rather than what it
means.

**This plan deletes that logic. It does not replace it.** Every entry below is
handled by [CHKARCH-TEXT-MATCHED-LOGIC](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TEXT-MATCHED-LOGIC):

1. **Write a test that fails** because of the code — an aliased import, a
   reformatted source, a shape the upstream fixture never contains.
2. **Delete the offending code.**
3. **Tell the user what you deleted and why**, and that the test is now failing.

Do not fix it, do not rewrite it, do not leave a TODO. **A failing test that pins
real incorrect behaviour is worth more than a passing fixture carried by logic
that does not analyse code.** Expect the rule count and the conformance number to
fall; report both drops plainly and never restore the code to hold either.
Deciding what gets built back is the user's, separately and deliberately.

The suggested AST replacements recorded below are **evidence of what the deleted
code failed to do**, kept so the user can scope a rebuild. They are not work
items in this plan.

## Current inventory {#LINESCANPLAN-INVENTORY}

The live inventory is the output of:

```bash
rg -n '\.lines\(\)' crates/basilisk-checker/src/rules
rg -n 'starts_with\("(class |def |async def |type |@|import |from )' \
  crates/basilisk-checker/src/rules
rg -n 'slice_span\(.*source' crates/basilisk-checker/src/rules
rg -n '\.(contains|starts_with|ends_with)\(' crates/basilisk-checker/src/rules
```

The first two find rules that reconstruct *statements* from lines. The third
finds rules that slice a span out of the source and pattern-match the resulting
**expression** text — the same defect one level down. The fourth is the widest
net and returns the most: text predicates appear in the large majority of rule
files, so the inventory below is a starting set, not the full extent.

Expression-text scanners:

- `aliases_type_statement.rs` — `is_invalid_rhs` classifies the RHS of a
  `type X = ...` statement by substring and prefix tests (`starts_with('[')`,
  `contains("lambda")`, `has_top_level_token(rhs, " or ")`, …). It is an
  allow-by-default list of textual shapes, so `type A = "the" + "thing"`,
  `type B = list["of genshin"]`, and `type D = list[int].attr` all pass
  silently, while a name containing the substring `lambda` is a false positive
  waiting to happen ([#379](https://github.com/Nimblesite/Basilisk/issues/379)).
  *What a real rule would have done:* type-expression grammar validation over the
  `StmtTypeAlias` value node — `Name`, dotted `Attribute` chains, `Subscript` of
  an allowed base, `BinOp(|)`, `None`, and forward-reference strings that
  themselves parse as type expressions; everything else rejected, including
  attribute access on a `Subscript`. Validation is eager at binding time — PEP 695
  lazy evaluation defers *name resolution*, never *well-formedness*.
- `aliases_implicit.rs` — carries a verbatim duplicate of the same
  `is_invalid_rhs` scanner, plus three further text heuristics: implicit
  aliases are detected by an uppercase-first-letter naming test
  ([#411](https://github.com/Nimblesite/Basilisk/issues/411)), `TypeAlias as X`
  imports are recovered by `match_indices` over raw import text duplicating the
  real name cascade ([#412](https://github.com/Nimblesite/Basilisk/issues/412)),
  and `looks_like_type_expression` gates on a character blacklist. The
  parameterization checks layered on top are fitted to the same fixture — the
  ParamSpec check is a shape guess that never locates the ParamSpec position
  ([#409](https://github.com/Nimblesite/Basilisk/issues/409)) and
  `is_assignable_to_bound` accepts every bound outside `int`/`float`/`complex`
  ([#410](https://github.com/Nimblesite/Basilisk/issues/410)). *What a real rule
  would have done:* validate the RHS expression node against the type-expression
  grammar and resolve alias-hood from binding information, never from name
  spelling. See [#408](https://github.com/Nimblesite/Basilisk/issues/408) and
  [`CONFORMANCE-INTEGRITY-AUDIT.md`](../CONFORMANCE-INTEGRITY-AUDIT.md).
- `returns_compatibility.rs` — builds the declared type from annotation source
  text via `InferredType::from_annotation`. Owned by
  [NARROWPLAN-ANNOTATION-RESOLUTION](CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-ANNOTATION-RESOLUTION)
  rather than this plan, but it is the same root cause and the same disposal.

Structural keyword scanners remain in:

- `generics_variance_inference/`
- `annotations_generators_2/mod.rs`
- `literals_literalstring_helpers.rs`
- `dataclasses_order.rs`
- `generics_defaults_referential_2.rs`

Other statement/body reconstruction remains in:

- `dataclasses_slots.rs` and `dataclasses_transform_class/helpers.rs`
- `protocols_subtyping.rs`
- `tuples_index_2.rs`
- `literals_semantics_2.rs`
- `specialtypes_never_2.rs`
- `generics_type_erasure.rs`

`rules/shared.rs::span_for_line` may read a line for diagnostic geometry. It must
not infer Python structure.

## Disposal {#LINESCANPLAN-DISPOSAL}

- [ ] For every inventory entry: failing test → delete → report. One rule per
  change, so each deletion and its drop are individually visible.
- [ ] The failing test must fail on **meaning, not spelling** — an aliased
  import, a reformatted source, or a construct the upstream suite omits. A test
  that only reproduces the fixture proves nothing.
- [ ] Record each deletion in the report: what went, which test now fails, and
  what the conformance run did afterwards. A drop is the expected outcome and is
  reported, never absorbed.
- [ ] Extend the inventory as the sweep widens. It was built from four queries
  and the fourth alone matches most rule files, so treat every unlisted rule as
  unaudited rather than clean.
- [ ] Never re-derive a deleted check from the same text predicates under a new
  name. If the analysis is worth having, the user scopes it as new work against
  the resolved model.

## Semantics-preserving mutation harness {#LINESCANPLAN-SEMANTIC-MUTATION}

Specified by
[CHKARCH-TESTING-SEMANTIC-MUTATION](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING-SEMANTIC-MUTATION)
and **not built**. It is the only gate that distinguishes a rule that analyses
code from a rule that matches text, and its absence is why this logic survived
every green run. Until it exists, no rule may be described as spec-implementing
on the strength of a fixture alone.

- [ ] Build the harness: re-run each rule test over semantically identical,
  textually different input (aliased imports, alternate import forms,
  reformatting, quote style, consistent renaming, statement reordering, comment
  churn) and require **byte-for-byte identical diagnostics**.
- [ ] Report coverage as a fraction of rules exercised, and **name the uncovered
  remainder**. A score over a hand-picked subset says nothing about the rest.
- [ ] Wire it into `make test`. A rule whose diagnostics move under mutation is
  handled by the three-step disposal above — never by teaching the harness to
  tolerate the difference, and never by mutating the expectation to match output.

## Enforcement {#LINESCANPLAN-ENFORCEMENT}

- [ ] Add a lint script that rejects new `.lines()` calls beneath
  `crates/basilisk-checker/src/rules/`, with a narrow allowlist for documented
  geometry helpers.
- [ ] Reject keyword-prefix parsing of Python structure in checker production
  code.
- [ ] Wire the lint into the existing lint/CI path; do not add a Make target.
- [ ] Keep the allowlist explicit and reviewed so it cannot grow silently.

## Acceptance {#LINESCANPLAN-ACCEPTANCE}

- No checker rule infers Python structure from raw source text.
- Docstrings, comments, and string literals containing `class`, `def`, `type`,
  decorators, or imports produce no structural diagnostics.
- The semantics-preserving mutation harness runs in `make test` and every
  surviving rule passes it.
- The set of deletions, the tests left failing behind them, and the resulting
  conformance drop are all on the record.
