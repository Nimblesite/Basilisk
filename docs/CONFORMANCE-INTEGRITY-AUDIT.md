# Conformance Integrity Audit — August 2026 [CHKARCH-CONFORMANCE-MUTATION]

An AST-preserving mutation of the python/typing conformance suite (consistent
typing-import renames such as `ClassVar as AuditClassVar`, plus whitespace
reformatting that changes neither semantics, line numbers, nor expected-error
markers — [sharkdp's harness](https://gist.github.com/sharkdp/3f3266fd9c67d22137e2b6c015c5f206),
vendored verbatim as `conformance/mutate_typing_conformance.py`) collapsed
Basilisk's conformance score from 141/141 to 28/141. A checker that reasons
about type structure cannot be affected by such mutations; the collapse proved
that many verdicts were fitted to the fixtures' exact source text.

This document records the audit's findings, what has been deleted and rebuilt,
the honest measured state, and the machinery that prevents recurrence. All
scores below are self-measured with the suite's OWN unmodified harness
(`conformance/src/main.py --only-run basilisk`, `conformance_automated`
verdicts) against a clean release build.

## Audit method

Every checker rule file was audited by parallel reviewers for verdicts that
read source text instead of structure, then adversarially verified with
reproduction inputs. **366 findings across 119 files; 239 graded CHEAT; 182
files clean.** Mechanism taxonomy:

| Mechanism | Meaning |
|---|---|
| A | Conformance-fixture identifiers hardcoded into production code |
| B | Substring/prefix matching on sliced source text |
| C | Line/statement reconstruction (indentation scans, `find(':')`) |
| D | `module.path` branching |
| E | Heuristics fitted to fixture shape (capitalisation, arg counts) |
| F | Hardcoded typing-symbol spellings compared against sliced text |

### Worst class — mechanism A (fixture text in production)

All deleted:

- `classes_classvar/*`: `ann.contains("CV[")` — the fixture's
  `from typing import ClassVar as CV` alias, hardcoded in four files.
- `types_parsing.rs:76`: any subscript head lowercasing to `l` treated as
  `typing.Literal` — existed only because one fixture spells `Literal as L`.
- `literals_literalstring_helpers.rs`: a line-scanner skip-list including the
  spelling `assert_type`.

### Structural failures the audit reproduced

- `NewType("X", L[7])` under a renamed `Literal` import was accepted
  (spec-invalid base, missed because the spelling check saw only `Literal[`).
- `NewType("X", list[MyClass])` was falsely flagged — `looks_like_typevar`
  treated ANY capitalised identifier as a TypeVar.
- `TypeIs[int | str]` vs `TypeIs[int|str]` compared unequal (raw text
  equality), producing a false incompatibility on legal reformatting.
- Whole-source byte scans for `self.` + `ClassVar` could not tell code from a
  docstring.

## What was rebuilt

One shared structural layer, used by every rewritten rule
([LINESCANPLAN-AST-MIGRATION]):

- `rules/shared/type_expr.rs` — the ONE type-expression judge
  (`is_type_expression`, `StringPolicy`, `ExprIndex` span→AST mapping).
- `rules/shared/typing_form.rs` — the ONE "does this denote `typing.X`?"
  question (`denotes`, `denotes_abc`, `subscript_of`, `strip_qualifiers`),
  answered through the module's import cascade
  ([TYPEINF-ANNOTATION-RESOLUTION]) so no verdict can depend on spelling.
- `rules/shared/runtime_names.rs` — the ONE "is this name a runtime value?"
  answer (structural, replacing capitalisation heuristics).
- `AnnotationResolver::spelling_denotes_from` — origin-aware member
  resolution (`typing`, `typing_extensions`, `collections.abc`).

Rewritten from text scanning to structural verdicts: `aliases_implicit`,
`aliases_type_statement`, `aliases_newtype`, `annotations_forward_refs`,
`qualifiers_annotated`, `qualifiers_final_annotation`, `classes_classvar`
(all five files), `literals_literalstring` (+helpers), `literals_semantics_2`,
`narrowing_typeis`, `tuples_index_2`, `tuples_type_compat` (annotation model +
rule; the `source.rs` line scanner is deleted), `specialtypes_never_2`,
`typeddicts_operations/type_consistency`, `dataclasses_transform_class`
(helpers + wiring), `directives_assert_type_2` (alias-aware tuple-union
equivalence), plus the shared `annotation_is_classvar`.

Three previously-red capability pins in
`crates/basilisk-checker/tests/type_expr_structural_tests.rs` are now green:
aliased-`Annotated` recognition, runtime names in module-var annotations, and
`assert_type` on tuple-union aliases.

## Honest measured state

| Suite | Before audit | After deletion + rebuild |
|---|---|---|
| Pristine python/typing (a490662) | 141/141 (carried by fitted text scans) | **141/141 — no fixture-fitted code remains on the pass path** |
| Mutated (527 renames + 729 reformats) | 28/141 | **32/141 (22.7%)** |

The pristine 141/141 now includes `tuples_type_compat` passing through a real
`assert_type` equivalence check rather than a spurious text-scan diagnostic
landing on the right lines.

### Why the mutated score is still low — the resolver layer

The remaining mutated failures are dominated by **`basilisk-resolver`'s
special-form collectors, which match callee spellings**
(`visitor/typevar.rs`, `type_alias.rs`, `typeddict.rs`, `assert_narrow.rs`,
…): under `NewType as AuditNewType`, `module.newtype_calls` comes back empty,
so every downstream structural rule loses its inputs and sibling rules emit
false positives. Migrating the resolver's recognition onto an import-alias
table is the next phase of this migration; the CI ratchet below exists to
force it and to hold every gain.

## Recurrence prevention

1. **Mutated-suite CI ratchet** — `make mutation-conformance` /
   `conformance/run_mutation_conformance.py` runs the vendored mutation
   harness + the official scorer in CI on every core change. The pass rate is
   a ratchet (`conformance/mutation_conformance_baseline.json`): it may only
   rise. A drop fails the build.
2. **Assertion-dense permutation tests** —
   `type_expr_structural_tests.rs` asserts exact flagged/unflagged name sets
   under renamed imports and whitespace mutation, with red pins for known
   gaps that must never be deleted or weakened.
3. **The spec-ID web** — every rewritten file references
   [LINESCANPLAN-AST-MIGRATION] and the audit issue (#408) at the point where
   text scanning used to live.

## Upstream status

python/typing removed the Basilisk adapter in `c43d32e` (typing#2330), so
`--only-run basilisk` is invalid at `main` HEAD. Both conformance runners pin
`a490662` (the last commit whose own harness carries the official
`BasiliskTypeChecker`) until the adapter is restored; the pins are marked to
be moved back to `main` at that moment. Published claims of conformance must
not exceed what the pinned, unmodified harness measures.
