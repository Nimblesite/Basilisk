# Eliminate Checker Line Scanning {#LINESCANPLAN-ELIMINATION}

> **Deletion complete (2026-08-06). Rebuild tracked elsewhere.** Every scanner
> catalogued below has been removed; the rules that depended on them are
> registered and inert. This plan is retained as the record of *what was deleted
> and why*. The work of putting AST-driven implementations back is
> [ASTREBUILD](CHECKER-AST-RECONSTRUCTION-PLAN.md#ASTREBUILD) — add nothing new
> here.
>
> The scope also turned out to be wider than "checker line scanning": the same
> mechanism was found throughout `basilisk-lsp` (move symbol, extract function,
> add `__all__`, auto-import placement, import rewriting on rename), which this
> plan never covered. Those are inventoried in
> [ASTREBUILD-INVENTORY-LSP](CHECKER-AST-RECONSTRUCTION-PLAN.md#ASTREBUILD-INVENTORY-LSP).
>
> The normative rule these scanners violated is now written down as
> [CHKARCH-RECOGNITION](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-RECOGNITION),
> with the LSP surface at
> [LSPARCH-ARCH-AST](../specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-AST) and
> [REFACTOR-AST](../specs/LSP-REFACTORING-SPEC.md#REFACTOR-AST).

> **Integrity status (2026-08-06):** The raw fixture counts in this plan are
> historical investigation records, not conformance percentages. Basilisk's
> former 100% claim is withdrawn and the current level is temporarily unknown
> while the scanners catalogued here are deleted and the affected logic is
> rebuilt structurally from the specification.

Checker rules must consume Ruff AST or `ResolvedModule` data. Reconstructing
Python structure with `source.lines()` plus `starts_with`, `find`, or `contains`
is parser duplication and can classify strings or comments as code.

The original docstring failure in `generics_syntax_scoping` is fixed and covered
by a regression test. This plan tracks the remaining rule-level scanners; line
geometry and suppression-comment parsing are the only permitted exceptions.

## Current inventory {#LINESCANPLAN-INVENTORY}

The live inventory is the output of:

```bash
rg -n '\.lines\(\)' crates/basilisk-checker/src/rules
rg -n 'starts_with\("(class |def |async def |type |@|import |from )' \
  crates/basilisk-checker/src/rules
rg -n 'slice_span\(.*source' crates/basilisk-checker/src/rules
```

The third query is new. The first two find rules that reconstruct *statements*
from lines, but they miss rules that slice a span out of the source and then
pattern-match the resulting **expression** text — which is the same defect one
level down, and is not caught by either keyword query.

Expression-text scanners — **RESOLVED** (see
[`CONFORMANCE-INTEGRITY-AUDIT.md`](../CONFORMANCE-INTEGRITY-AUDIT.md)). The
shared structural judge lives in `rules/shared/type_expr.rs`
([LINESCANPLAN-AST-MIGRATION]); `tests/type_expr_structural_tests.rs` pins the
verdicts under import renames and whitespace mutation, with `red_pin_` tests
holding the remaining honest gaps open:

- `aliases_type_statement.rs` — DONE. The rule validates the `StmtTypeAlias`
  value node through the shared judge; attribute access on a `Subscript`
  (`list[int].attr`) is rejected
  ([#379](https://github.com/Nimblesite/Basilisk/issues/379)).
- `aliases_implicit.rs` — DONE. `is_invalid_rhs`, its private
  `has_top_level_token` / `paren_has_top_level_comma` copies, the
  `match_indices("TypeAlias as ")` import scan
  ([#412](https://github.com/Nimblesite/Basilisk/issues/412)), the
  uppercase-first-letter implicit-alias heuristic
  ([#411](https://github.com/Nimblesite/Basilisk/issues/411)), the
  `looks_like_type_expression` character blacklist, the ParamSpec shape guess
  ([#409](https://github.com/Nimblesite/Basilisk/issues/409)), and the
  three-name `is_assignable_to_bound`
  ([#410](https://github.com/Nimblesite/Basilisk/issues/410)) are all deleted.
  Alias-hood resolves through the annotation cascade; type parameters,
  ParamSpec positions (with the PEP 612 single-parameter auto-wrap), variadic
  arity (PEP 646), and bounds (via the module subtyping context, abstaining on
  unknown names) are computed from the AST.
- `annotations_forward_refs/type_checks.rs` and
  `qualifiers_annotated/helpers.rs` — DONE. The `is_invalid_type_annotation` /
  `is_invalid_type_expr` text families (including the last two
  `starts_with("eval(")` copies) are deleted; both rules judge annotation
  nodes via a span→AST index, and forward-reference strings are parsed and
  judged as expressions. The orphaned `text_scan::contains_top_level_comma` /
  `paren_has_top_level_comma` helpers are deleted with them
  ([#408](https://github.com/Nimblesite/Basilisk/issues/408)).
- `returns_compatibility.rs` — builds the declared type from annotation source
  text via `InferredType::from_annotation`. Owned by
  [NARROWPLAN-ANNOTATION-RESOLUTION](CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-ANNOTATION-RESOLUTION)
  rather than this plan, but it is the same root cause and the same fix shape.

Structural keyword scanners **were** in the list below. All are now deleted; the
rules are registered and inert pending
[ASTREBUILD-PHASE-RULES](CHECKER-AST-RECONSTRUCTION-PLAN.md#ASTREBUILD-PHASE-RULES):

- `generics_variance_inference/` — six submodules deleted
- `annotations_generators_2/` — `annotation.rs`, `type_check.rs`, `yield_scan.rs`
  deleted; the last was a hand-rolled Python lexer hunting the characters
  `yield`
- `literals_literalstring_helpers.rs`
- `dataclasses_order.rs`
- `generics_defaults_referential_2.rs`
- `dataclasses_slots.rs` and `dataclasses_transform_class/` — the first keeps
  its resolver-derived half
- `protocols_subtyping.rs`, `tuples_index_2.rs`, `literals_semantics_2.rs`,
  `specialtypes_never_2.rs`
- `generics_type_erasure.rs` — keeps its `module_attr_assignments` half

Also deleted outside this plan's original scope: `basilisk-checker/src/ownership.rs`,
whose own module documentation conceded its scan was textual and matched inside
comments and strings.

`rules/shared/text_scan.rs` retains `leading_indent`, `span_for_line`, and
`split_top_level_commas` — line **geometry** for diagnostic placement, inferring
no Python structure. `identifiers_followed_by` is deleted.

## AST migration {#LINESCANPLAN-AST-MIGRATION}

Superseded. Rebuilding on the AST is
[ASTREBUILD-PHASES](CHECKER-AST-RECONSTRUCTION-PLAN.md#ASTREBUILD-PHASES), which
covers the resolver, the checker rules, the annotation-text layer, and the LSP
in dependency order. One item completed under this plan is kept for the record:

- [x] `aliases_type_statement::is_invalid_rhs` replaced with AST type-expression
  validation, covering operators other than `|`, call expressions outside the
  sanctioned special forms, comparisons, comprehensions, and literal displays.

One historical measurement, retained as fixture evidence and **not** a
conformance level: a raw run after the first rewrite returned 140/141;
`tuples_type_compat` requires either `len()`/`match` tuple narrowing or an
`assert_type` mismatch verdict on alias-typed values (red-pinned in
`tests/type_expr_structural_tests.rs`), and its lines had previously been
"passed" by a spurious text-scan diagnostic — a scanner producing a *correct*
count for a *wrong* reason, which is the whole reason raw counts are not
evidence.

## Enforcement {#LINESCANPLAN-ENFORCEMENT}

The rule is now normative in the specification
([CHKARCH-RECOGNITION-BANNED](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-RECOGNITION-BANNED)),
which is what review enforces against. Automated enforcement remains open:

- [ ] Reject new `.lines()` calls and keyword-prefix parsing of Python structure
  in production code across **all** crates — not only `basilisk-checker/rules`,
  which is the scoping error that let the LSP accumulate the same mechanism
  unnoticed.
- [ ] Wire it into the existing lint/CI path; do not add a Make target.
- [ ] Keep the allowlist explicit, documented, and reviewed so it cannot grow
  silently. Current lawful entries: line geometry, `# basilisk:` directive
  parsing, and Basilisk's own rendered stub-signature output.

A lint cannot catch the semantic form of this defect — the deleted
`denotes(expr, "TypeVar")` API would pass any text-based check. Behavioural
tests are the real gate:
[CHKARCH-RECOGNITION-VERIFY](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-RECOGNITION-VERIFY).

## Acceptance {#LINESCANPLAN-ACCEPTANCE}

- No production code in any crate infers Python structure from raw source.
- Docstrings, comments, and string literals containing `class`, `def`, `type`,
  decorators, or imports produce no structural diagnostics and no code actions.
- The enforcement lint above is wired in.
- Delete this plan once those hold; the rebuild's own gate is
  [ASTREBUILD-ACCEPTANCE](CHECKER-AST-RECONSTRUCTION-PLAN.md#ASTREBUILD-ACCEPTANCE).
