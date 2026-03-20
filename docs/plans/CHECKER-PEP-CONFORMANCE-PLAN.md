# PEP Conformance — Plan

> **Score**: 135/146 (92.5%)
> **Tests**: `crates/basilisk-cli/tests/conformance/`
> **Status CSV**: `conformance/conformance_status.csv`
> **Run**: `./conformance/conformance.sh` or `cargo test --test conformance_tests -- --nocapture`

---

## COMPLETED

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

## TODO — 11 failing files remaining

### Protocols (1 file, 17 missed, 13 FP)

- [ ] `protocols_definition.py` (17 missed, 13 FP) — structural subtyping, method signature compat

### Generics — TypeVarTuple (2 files, 15 missed)

- [ ] `generics_typevartuple_basic.py` (6 missed) — deep type inference for TVT matching
- [ ] `generics_typevartuple_args.py` (9 missed) — `*args` typing with TypeVarTuple

### Generics — ParamSpec (3 files, 30 missed)

- [ ] `generics_paramspec_components.py` (16 missed) — `P.args` / `P.kwargs`
- [ ] `generics_paramspec_semantics.py` (9 missed, 1 FP) — constraint solving
- [ ] `generics_paramspec_specialization.py` (5 missed, 3 FP) — concrete specialization

### Type Aliases (1 file, 22 missed)

- [ ] `aliases_typealiastype.py` (22 missed, 3 FP) — `TypeAliasType` call-based aliases

### Constructors (1 file, 12 missed)

- [ ] `constructors_callable.py` (12 missed) — callable as constructor

### Dataclasses (1 file, 6 missed)

- [ ] `dataclasses_transform_converter.py` (6 missed, 3 FP) — `converter` semantics

### TypedDict (2 files, 27 missed)

- [ ] `typeddicts_extra_items.py` (18 missed, 13 FP) — `extra_items` kwarg
- [ ] `typeddicts_readonly_inheritance.py` (9 missed, 2 FP) — readonly + inheritance
