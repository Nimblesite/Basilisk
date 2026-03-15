# PEP Conformance — Plan

> **Score**: 130/146 (89.0%)
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

## TODO — 16 failing files remaining

### Protocols (2 files, 23 missed, 14 FP)

- [ ] `protocols_definition.py` (17 missed, 13 FP) — structural subtyping, method signature compat
- [ ] `protocols_generic.py` (6 missed, 1 FP) — generic protocol assignability

### Generics — TypeVarTuple (2 files, 15 missed)

- [ ] `generics_typevartuple_basic.py` (6 missed) — deep type inference for TVT matching
- [ ] `generics_typevartuple_args.py` (9 missed) — `*args` typing with TypeVarTuple

### Generics — ParamSpec (3 files, 30 missed)

- [ ] `generics_paramspec_components.py` (16 missed) — `P.args` / `P.kwargs`
- [ ] `generics_paramspec_semantics.py` (9 missed) — constraint solving
- [ ] `generics_paramspec_specialization.py` (5 missed, 3 FP) — concrete specialization

### Generics — Defaults (1 file, 2 missed)

- [ ] `generics_defaults_referential.py` (2 missed) — TypeVar defaults referencing other TypeVars

### Type Aliases (1 file, 22 missed)

- [ ] `aliases_typealiastype.py` (22 missed, 3 FP) — `TypeAliasType` call-based aliases

### Callables (2 files, 15 missed)

- [ ] `callables_kwargs.py` (9 missed, 1 FP) — `Unpack[TypedDict]` kwargs validation
- [ ] `callables_protocol.py` (6 missed, 2 FP) — callback protocol matching

### Constructors (1 file, 12 missed)

- [ ] `constructors_callable.py` (12 missed) — callable as constructor

### Dataclasses (1 file, 6 missed)

- [ ] `dataclasses_transform_converter.py` (6 missed, 3 FP) — `converter` semantics

### TypedDict (3 files, 36 missed)

- [ ] `typeddicts_extra_items.py` (19 missed, 11 FP) — `extra_items` kwarg
- [ ] `typeddicts_readonly_inheritance.py` (9 missed, 2 FP) — readonly + inheritance
- [ ] `typeddicts_type_consistency.py` (8 missed, 2 FP) — type consistency
