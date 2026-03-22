# Plan: Eliminate False Positives in PEP Conformance Suite

## Context

The conformance suite tracks ~230 false positives across 60+ files. False positives are diagnostics Basilisk reports on lines that have NO `# E` annotation — meaning the typing spec says these lines are **valid code** but Basilisk incorrectly flags them. This erodes user trust and blocks adoption.

The conformance harness (`crates/basilisk-cli/tests/conformance_tests.rs:272-275`) counts FPs but only prints verbose output for **missed** errors, not for FPs. We need to fix the checker rules that over-report.

---

## Step 0: Add FP Verbose Reporting to Conformance Harness

**File**: `crates/basilisk-cli/tests/conformance_tests.rs`

Add a debug print (like the existing missed-lines print at line 284) that shows:
- Which lines are FPs (diagnostic on unannotated line)
- Which rule code fired on each FP line

This gives us the exact rule-to-line mapping needed to fix each FP surgically. Currently we only know FP counts per file, not which rules cause them.

```rust
// After line 275, add:
if false_positives > 0 {
    let fp_details: Vec<(usize, String)> = /* collect (line, rule) for FP lines */;
    println!("  FP {file_name}: count={false_positives} lines={fp_details:?}");
}
```

This requires threading the rule code through with the line number (currently `diag_lines` is `HashSet<usize>` — change to store `(line, rule_code)` pairs).

**Verification**: Run `cargo test --test conformance_tests -- --nocapture` and inspect FP output.

---

## Step 1: E0104 — Recursive Type Aliases (est. ~20 FP eliminated)

**File**: `crates/basilisk-checker/src/rules/e0104.rs`
**Conformance file**: `aliases_recursive.py` (20 FP)

**Problem**: E0104 flags ALL cyclical type aliases, but PEP 695 explicitly allows recursive aliases that have a terminating base case (e.g., `Json = Union[None, int, str, list["Json"]]`). Only truly infinite cycles should be flagged (e.g., `RecursiveUnion: TypeAlias = Union["RecursiveUnion", int]` where the recursive branch wraps nothing new).

**Fix**:
- Check if the alias is a PEP 695 `type` statement → always allow recursion
- For `TypeAlias`-annotated assignments, check if the Union has at least one non-recursive branch that doesn't reference any alias in the cycle → allow (it's a valid recursive type with a base case)
- Only flag aliases where the recursion cannot terminate (all paths lead back to self with no widening)

The test file shows exactly which should error: only lines 72 and 75 (`RecursiveUnion` and `MutualReference1/2`). Lines 14, 24, 30, 42, 58, 65 are valid recursive aliases.

---

## Step 2: E0136 — Callable Subtyping (est. ~25 FP eliminated)

**File**: `crates/basilisk-checker/src/rules/e0136.rs`
**Conformance file**: `callables_subtyping.py` (25 FP)

**Problem**: The `is_subtype` function uses text-based type comparison and only handles the numeric hierarchy (`int`/`float`/`complex`/`bool`) and exact name matches. It doesn't understand:
- `object` as universal supertype for all parameters
- Union types in parameters (contravariance: `Callable[[int|str], None]` accepts `Callable[[int], None]`)
- `*args`/`**kwargs` parameter compatibility
- Default parameter arity tolerance
- Protocol `__call__` ↔ `Callable` equivalence

**Fix**: Enhance `is_subtype` to handle Union decomposition, `object`, and `None`/`Optional`. Add `*args`/`**kwargs` awareness to parameter count checking.

---

## Step 3: E0014 — Assignment Type Mismatch (est. ~30 FP eliminated)

**Files**: `crates/basilisk-checker/src/rules/e0014/mod.rs`, `literal_parse.rs`, `tuple_check.rs`
**Top conformance files**: `callables_annotation.py` (17 FP), `tuples_type_compat.py` (17 FP), `specialtypes_any.py` (7 FP), `typeddicts_readonly_consistency.py` (5 FP)

**Problem**: E0014 uses text-based annotation parsing and literal inference. It over-reports when:
- Annotation is a `Callable[...]` or Protocol name (should defer to E0136/E0140)
- Annotation is a bare generic (`list`, `dict` without `[...]`) which implicitly means `Any`
- Annotation is a recursive type alias name
- Annotation involves `tuple[T, ...]` homogeneous form
- RHS is a function call, parameter reference, or complex expression (not a simple literal)

**Fix**: Add suppression guards:
1. Skip when annotation text references a known type alias from `module.type_alias_defs`
2. Skip when annotation starts with `Callable` (defer to E0136/E0140)
3. Skip when annotation is a bare generic name without `[...]`
4. Improve tuple annotation handling for variadic forms

---

## Step 4: E0093 + TypedDict Rules (est. ~15 FP eliminated)

**Files**: `crates/basilisk-checker/src/rules/e0093/mod.rs`, `type_consistency.rs`
**Conformance files**: `typeddicts_extra_items.py` (13 FP), `typeddicts_readonly.py` (2 FP), `typeddicts_required.py` (3 FP), `typeddicts_readonly_consistency.py` (5 FP), `typeddicts_readonly_update.py` (3 FP)

**Problem**: TypedDict checking has gaps around:
- `extra_items` (PEP 728) — TypedDicts can allow extra items with a declared type
- `ReadOnly` fields (PEP 705) — read operations on ReadOnly fields are valid
- TypedDict-to-TypedDict structural assignment compatibility
- Inheritance field resolution (transitive required/optional status)

**Fix**:
- Add `extra_items` support to E0093's key validation
- Handle ReadOnly semantics (reads are always valid, only mutation blocked)
- Build transitive field resolution for TD inheritance chains

---

## Step 5: Protocol Rules — E0121, E0110, E0133 (est. ~25 FP eliminated)

**Files**: `crates/basilisk-checker/src/rules/e0121.rs`, `e0110.rs`, `e0133.rs`
**Conformance files**: `protocols_definition.py` (13 FP), `protocols_subtyping.py` (7 FP), `protocols_merging.py` (2 FP), `protocols_recursive.py` (2 FP), `callables_protocol.py` (1 FP)

**Problem**:
- E0121: Protocol conformance checking misses inherited methods, builtin type dunders, and attribute-only protocol members
- E0110/E0133: Variance inference doesn't exempt `__init_subclass__`, `@property`, or explicit `self: X[T]` annotations

**Fix**:
- E0121: Walk full MRO for method resolution, add builtin dunder table, check attributes
- E0110: Add more exempt methods, detect `@property` as output-only
- E0133: Handle PEP 695 inline TypeVar declarations

---

## Step 6: Remaining Scatter (est. ~20 FP eliminated)

**Rules**: E0013, E0036, E0053, E0061, E0045, E0147, and other low-count rules
**Files**: `exceptions_context_managers.py` (6 FP), `enums_members.py` (7 FP), `narrowing_typeguard.py` (5 FP), `narrowing_typeis.py` (4 FP), `literals_interactions.py` (4 FP), many files with 1-3 FP

These require per-rule investigation after Step 0 gives us exact FP-to-rule mapping. Common themes:
- Context manager `__exit__` return type narrowing
- Enum member/nonmember decorator semantics
- TypeGuard/TypeIs narrowing edge cases
- Literal type interactions

---

## Step 7: Type Narrowing and Full Inference Engine (est. ~125 FP eliminated)

> **Full plan**: [CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md](CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md)
> **Spec**: [CHECKER-TYPE-INFERENCE-SPEC.md](../specs/CHECKER-TYPE-INFERENCE-SPEC.md)

The remaining ~125 FPs cannot be fixed without fundamental engine work. The checker currently uses text-based annotation parsing and literal-only RHS inference. It has no narrowing engine, no TypeVar constraint solver, and no class hierarchy resolution. See the dedicated plan for full details.

---

## Execution Order

### Phase 1: Rule-Specific Fixes (Steps 0-6) — MOSTLY COMPLETE

| Step | Est. FPs Fixed | Effort | Risk | Status |
|------|---------------|--------|------|--------|
| 0. FP verbose reporting | 0 (tooling) | Low | None | DONE |
| 1. E0104 recursive aliases | ~20 | Low | Low | DONE (2 FPs fixed) |
| 2. E0136 callable subtyping | ~25 | Medium | Medium | Pending |
| 3. E0014 assignment mismatch | ~110 | Medium | Medium | **DONE (108 FPs fixed)** |
| 4. E0093 TypedDict | ~15 | Medium | Low | Partial (9 FPs fixed, 7 remain) |
| 5. E0121/E0110/E0133 protocols | ~25 | High | Medium | Partial (12 FPs fixed) |
| 6. E0013 return type mismatch | ~7 | Low | Low | **DONE (7 FPs fixed)** |
| 7. File-level `# type: ignore` | ~1 | Low | None | **DONE** |
| 8. Remaining scatter | ~20 | High | Low | In Progress |

### Phase 2: Type Narrowing and Full Inference — DONE

Type narrowing engine, expression inference, constraint solver, and subtyping context all implemented. See [CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md](CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md). FPs reduced from 57 to 18.

---

## Verification

After each step:
1. Run `cargo test --test conformance_tests -- --nocapture`
2. Check that `conformance_status.csv` FP counts decrease
3. Verify no new test failures (missed count must not increase)
4. Run `cargo clippy` and `cargo fmt`

---

## TODO

> **Total: 18 FPs** (down from 251→177→57→18, as of 2026-03-21)

### FP Breakdown by Rule (from conformance test verbose output)

| Rule | FPs | Files |
|------|-----|-------|
| BSK-E0094 | 2 | generics_self_usage (2) |
| BSK-E0093 | 2 | typeddicts_final (1), typeddicts_readonly_update (1) |
| BSK-E0140 | 2 | callables_kwargs (1), typeddicts_readonly_kwargs (1) |
| BSK-E0014 | 2 | dataclasses_transform_converter (1), directives_type_checking (1) |
| BSK-E0149 | 1 | generics_syntax_scoping (1, triple-reported) |
| BSK-E0141 | 1 | typeddicts_extra_items (1) |
| BSK-E0132 | 1 | typeddicts_extra_items (1) |
| BSK-E0139 | 1 | generics_typevartuple_specialization (1) |
| BSK-E0143 | 1 | namedtuples_usage (1) |
| BSK-E0115 | 1 | directives_deprecated (1) |
| BSK-E0112 | 1 | narrowing_typeguard (1) |
| BSK-E0099 | 1 | generics_self_protocols (1) |
| BSK-E0092 | 1 | generics_paramspec_specialization (1) |
| BSK-E0078 | 1 | generics_self_usage (1) |
| BSK-E0069 | 1 | dataclasses_transform_func (1) |

### Completed (120+ FPs eliminated, 177 -> 57 as of 2026-03-21)

- [x] Step 0: Add FP verbose reporting to conformance harness — `diag_line_rules` HashMap in `conformance_tests.rs`
- [x] BSK-E0014: bare generics (`list`, `dict`, `set`, `tuple`) + `complex` type recognition — 7 FPs fixed
- [x] BSK-E0104: recursive alias cycle detection — allow container-wrapped recursion — 2 FPs fixed
- [x] BSK-E0061: enum `assert_type` — only flag when first arg is enum-typed param — 8 FPs fixed
- [x] BSK-E0111: constructor calls — add builtin base classes, dataclass inheritance, `__init_subclass__` — 9 FPs fixed
- [x] BSK-E0149: PEP 695 type param extraction — fixed `extract_pep695_type_params` to check `=` for `type` statements, preventing RHS subscripts from being treated as type params — 18 FPs fixed
- [x] BSK-E0093: TypedDict `Required`/`NotRequired`/`ReadOnly` — added `is_field_required()` helper and `strip_td_wrappers()` for value type comparison — 9 FPs fixed
- [x] BSK-E0110: protocol variance — treat invariant containers as variance-neutral; add `tuple`/`Tuple` to covariant containers — 5 FPs fixed
- [x] BSK-E0121: protocol conformance — check class attributes (not just methods) for protocol member satisfaction — 7 FPs fixed
- [x] BSK-E0130: TypeVar scoping — skip docstrings, comments, and multi-line function signature continuations — 6 FPs fixed
- [x] BSK-E0014: **massive FP elimination** — suppress when RHS is non-literal (variable/param/call) and involves Named/Callable/Tuple types; suppress when declared type is Named (unresolved alias); `contains_unresolvable` recursive check; `# type: ignore` line-level support — **101 FPs fixed**
- [x] BSK-E0013: return type mismatch — `contains_named` recursive check for declared types with Named inside unions/tuples; `is_base_to_literal` for Bool→Literal[True/False] and Int→Literal[N] etc. — **7 FPs fixed**
- [x] BSK-E0053: `assert_type` — variadic tuple equivalence in `types_match` — **4 FPs fixed**
- [x] File-level `# type: ignore` support in `suppression.rs` — detect standalone `# type: ignore` before executable code and suppress all diagnostics — **1 FP fixed**

### Remaining: 18 FPs across 15 files (scattered, low-count rules)

All remaining FPs are 1-2 per rule — no single rule dominates. See updated breakdown table above.

**Notable**: E0094 (2), E0093 (2), E0140 (2), E0014 (2) are the only rules with >1 FP. All others have exactly 1.
