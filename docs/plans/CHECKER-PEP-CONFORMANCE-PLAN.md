# PEP Conformance — Plan {#PEPPLAN}

> ⚠️ **SUPERSEDED SCORES BELOW.** Every percentage in this plan (e.g. "137/146,
> 93.84%", category "100%" rows) came from a since-removed in-repo harness that
> excluded 9 diagnostic codes and ignored false positives. The score is now
> computed by the **unmodified `python/typing` calculator** (pinned `268d0c4e`,
> `conformance/score.py`, see [CHKARCH-CONFORMANCE]) with the `basilisk` binary run
> with **EVERY rule enabled** — no config, no `basilisk.json`, no exceptions. The
> honest current number is **68/146 = 46.6%** (errors+warnings, strictest):
> **265 false positives, 0 missed required errors**. The checker catches every
> required error; each failing fixture fails purely on false positives from
> strict-by-default house-style rules (require-annotation E0001/E0002/E0004,
> missing-`@override` E0025, explicit-`Any` W0014, redundant-annotation W0050)
> firing on spec-valid code where the spec treats unannotated as inferred, not an
> error.
>
> **There is NO "spec-conformance mode" any more** — see CHKARCH-CONFORMANCE-MODE,
> which now documents that no such mode exists. Disabling ANY conformance rule for
> scoring is **FORBIDDEN** — a punishable offence. The only legitimate path to
> 100% is fixing the checker so its strict defaults stop firing on spec-valid code,
> with every rule still enabled — never by disabling a rule.
>
> **HISTORY (not the current figure):** the last honest score was 59/146 = 40.4%
> (285 FPs) at PR #183. PRs #184/#185/#191 inflated the reported number to a fake
> 100% by writing a `basilisk.json` that disabled those 6 house rules at score time
> (the so-called "spec-conformance mode") — the checker was not made smarter, the
> false positives were merely hidden. That disabling has been removed. Genuine
> progress over that span was real but modest: 40.4% → 46.6%. Treat the figures
> below as historical task notes, not the live score.
>
> **Run**: `make conformance` · **Status CSV**: `conformance/conformance_status.csv`
> · **Tests**: `crates/basilisk-cli/tests/conformance/`

---

## COMPLETED {#PEPPLAN-COMPLETED}

- [x] E0083: TypeVarTuple inside `tuple[...]` annotations + return annotations
- [x] E0086: Multiple unpacks in `tuple[...]` type aliases (lines 121,122)
- [x] E0139: `TA11[*Ts2]` invalid specialization — FLIPPED `generics_typevartuple_specialization.py`
- [x] E0149: isinstance in if-test condition — FLIPPED `aliases_type_statement.py`
- [x] E0150: Dead branch variables for version/platform guards — FLIPPED `directives_version_platform.py`
- [x] E0130: Variance inference (PEP 695 + `infer_variance`) — FLIPPED `generics_variance_inference.py` + `generics_syntax_infer_variance.py`
- [x] Directives category: 100% (10/10)
- [x] TypeForms category: 100% (1/1)
- [x] Aliases category: 6/7 (85.7%)
- [x] Generics category: 23/30 (76.7%)
- [x] `protocols_generic.py` — generic protocol assignability — FLIPPED
- [x] `typeddicts_type_consistency.py` — TypedDict type consistency — FLIPPED
- [x] E0153: Constructor-to-callable conversion + call validation — FLIPPED `constructors_callable.py` ([CHKARCH-DIAG-CTOR-CALLABLE](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CTOR-CALLABLE))
- [x] E0038/E0056/E0093/E0014: PEP 705 `ReadOnly` `TypedDict` inheritance — FLIPPED `typeddicts_readonly_inheritance.py` ([CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE)). Transitive `TypedDict` recognition + effective merged schema cleared the E0014 dict-literal false positives across the readonly suite (126 → 120 total FPs).

## TODO — 23 failing files remaining {#PEPPLAN-TODO}

### Protocols (2 files) {#PEPPLAN-TODO-PROTOCOLS}

- [ ] `protocols_definition.py` (19 missed) — structural subtyping, method signature compat
- [ ] `protocols_subtyping.py` (6 missed) — protocol assignability

### Generics — TypeVarTuple (2 files) {#PEPPLAN-TODO-TYPEVARTUPLE}

- [ ] `generics_typevartuple_basic.py` (6 missed) — deep type inference for TVT matching
- [ ] `generics_typevartuple_args.py` (9 missed) — `*args` typing with TypeVarTuple

### Generics — ParamSpec (4 files) {#PEPPLAN-TODO-PARAMSPEC}

- [ ] `generics_paramspec_basic.py` (1 missed) — basic ParamSpec usage
- [ ] `generics_paramspec_components.py` (16 missed) — `P.args` / `P.kwargs`
- [ ] `generics_paramspec_semantics.py` (9 missed) — constraint solving
- [ ] `generics_paramspec_specialization.py` (5 missed, 1 FP) — concrete specialization

### Type Aliases (3 files) {#PEPPLAN-TODO-ALIASES}

- [ ] `aliases_recursive.py` (9 missed) — recursive alias handling
- [ ] `aliases_type_statement.py` (2 missed) — PEP 695 type statement edge cases
- [ ] `aliases_typealiastype.py` (22 missed) — `TypeAliasType` call-based aliases

### Callables (2 files) {#PEPPLAN-TODO-CALLABLES}

- [ ] `callables_annotation.py` (4 missed) — callable annotation edge cases
- [ ] `callables_subtyping.py` (30 missed) — callable subtyping rules

### Constructors {#PEPPLAN-TODO-CONSTRUCTORS}

- [x] `constructors_callable.py` — constructor-to-callable conversion (constructors_callable) — DONE

### Dataclasses (1 file) {#PEPPLAN-TODO-DATACLASSES}

- [ ] `dataclasses_transform_converter.py` (9 missed, 1 FP) — `converter` semantics

### Directives (2 files) {#PEPPLAN-TODO-DIRECTIVES}

- [ ] `directives_assert_type.py` (1 missed) — assert_type edge case
- [ ] `directives_deprecated.py` (1 missed) — deprecated detection

### Generics — Self (1 file) {#PEPPLAN-TODO-SELF}

- [ ] `generics_self_usage.py` (1 missed, 1 FP) — Self type usage

### Special Types (1 file) {#PEPPLAN-TODO-SPECIALTYPES}

- [ ] `specialtypes_none.py` (1 missed) — None type edge case

### Tuples (1 file) {#PEPPLAN-TODO-TUPLES}

- [ ] `tuples_type_compat.py` (6 missed) — tuple type compatibility

### TypedDict (3 files) {#PEPPLAN-TODO-TYPEDDICT}

- [ ] `typeddicts_extra_items.py` (18 missed, 7 FP) — `extra_items` kwarg (PEP 728)
- [x] `typeddicts_readonly_inheritance.py` — PEP 705 `ReadOnly`/`Required`/`NotRequired` redeclaration legality + transitive inheritance — DONE ([CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE))
