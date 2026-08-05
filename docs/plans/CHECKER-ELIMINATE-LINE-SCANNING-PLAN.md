# Eliminate Checker Line Scanning {#LINESCANPLAN-ELIMINATION}

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

## AST migration {#LINESCANPLAN-AST-MIGRATION}

- [ ] Replace class/function/type-alias discovery with the corresponding
  `ResolvedModule` collections and Ruff AST spans.
- [ ] Replace indentation-based body boundaries with AST statement/body ranges.
- [ ] Replace operator, mutation, and call parsing with structured expression or
  call records; extend the resolver when the required node is not exposed.
- [ ] Add a string/comment regression fixture for each migrated rule before
  deleting its scanner.
- [x] Replace `aliases_type_statement::is_invalid_rhs` with AST type-expression
  validation, covering the reported cases above plus operators other than `|`,
  call expressions outside the sanctioned special forms, comparisons,
  comprehensions, and literal displays.
- [ ] Preserve the exact diagnostics for real code and restore conformance to
  141/141 with zero missed errors and zero false positives. Post-rewrite the
  honest score is 140/141: `tuples_type_compat` requires either len()/match
  tuple narrowing or an assert_type mismatch verdict on alias-typed values
  (red-pinned in `tests/type_expr_structural_tests.rs`); its lines were
  previously "passed" by a spurious text-scan diagnostic.

Migrate `generics_variance_inference` first: it owns the largest cluster of raw
line and keyword scans. Then take `aliases_type_statement`, which is the only
inventory entry with a confirmed user-visible miss. Then remove the smaller
body scanners in the inventory above.

## Enforcement {#LINESCANPLAN-ENFORCEMENT}

- [ ] Add a lint script that rejects new `.lines()` calls beneath
  `crates/basilisk-checker/src/rules/`, with a narrow allowlist for documented
  geometry helpers.
- [ ] Reject keyword-prefix parsing of Python structure in checker production
  code.
- [ ] Wire the lint into the existing lint/CI path; do not add a Make target.
- [ ] Keep the allowlist explicit and reviewed so it cannot grow silently.

## Acceptance {#LINESCANPLAN-ACCEPTANCE}

- No checker rule infers Python structure from raw lines.
- Docstrings, comments, and string literals containing `class`, `def`, `type`,
  decorators, or imports produce no structural diagnostics.
- Focused rule tests, `make lint`, `make test`, and the live conformance harness
  pass without weakening any ratchet.
