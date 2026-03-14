# PEP Conformance — Plan

> **Score**: 125/146 (85.6%)
> **Tests**: `crates/basilisk-cli/tests/conformance/`
> **Status CSV**: `conformance/conformance_status.csv`
> **Run**: `./conformance/conformance.sh` or `cargo test --test conformance_tests -- --nocapture`

---

## Completed

- [x] E0130: Module-level type alias TypeVar usage — skip assignments (PEP 484/613)
- [x] E0130: `Protocol[T]` binding — treat like `Generic[T]`
- [x] E0130: Multi-line function signatures — collect full sig for TypeVar extraction
- [x] E0111: Skip `is_dataclass` and `is_typed_dict` classes (synthesized `__init__`)
- [x] E0092: `Expr::Starred` handling in `collect_name_refs_from_expr`
- [x] E0111: NamedTuple constructor arg count validation
- [x] E0148: Class inheritance in constrained TypeVar resolution
- [x] E0093/E0014: TypedDict `extra_items` kwarg handling in resolver
- [x] FP reduction: 435 → 274 unexpected diagnostics

---

## TODO — 21 failing test files

### Protocols (2 files, ~40 missed diagnostics)

- [ ] `protocols_definition.py` (missed: 17, FP: 13) — structural subtyping: attrs satisfy properties, method signature compatibility
- [ ] `protocols_generic.py` (missed: 6, FP: 1) — variance tracking in generic protocol type assignability

### Generics — TypeVarTuple (3 files, ~20 missed)

- [ ] `generics_typevartuple_basic.py` (missed: 8) — unpack semantics, TypeVarTuple in generic classes
- [ ] `generics_typevartuple_args.py` (missed: 9) — *args typing with TypeVarTuple
- [ ] `generics_typevartuple_specialization.py` (missed: 3, FP: 2) — TypeVarTuple concrete specialization

### Generics — ParamSpec (3 files, ~30 missed)

- [ ] `generics_paramspec_components.py` (missed: 16) — `P.args`, `P.kwargs` component access
- [ ] `generics_paramspec_semantics.py` (missed: 9, FP: 1) — ParamSpec constraint solving
- [ ] `generics_paramspec_specialization.py` (missed: 5, FP: 3) — ParamSpec concrete specialization

### Generics — Variance (2 files, ~39 missed)

- [ ] `generics_variance_inference.py` (missed: 23, FP: 2) — auto-variance from covariant/contravariant/invariant positions
- [ ] `generics_syntax_infer_variance.py` (missed: 16, FP: 1) — PEP 695 syntax variance inference

### Generics — Defaults (1 file, 2 missed)

- [ ] `generics_defaults_referential.py` (missed: 2) — TypeVar defaults referencing other TypeVars in generic constructors

### Type Aliases (2 files, ~32 missed)

- [ ] `aliases_type_statement.py` (missed: 10, FP: 4) — PEP 695 `type` statement aliases
- [ ] `aliases_typealiastype.py` (missed: 22, FP: 3) — `TypeAliasType` call-based aliases

### Callables (2 files, ~15 missed)

- [ ] `callables_kwargs.py` (missed: 9, FP: 1) — `**kwargs` type checking in callable signatures
- [ ] `callables_protocol.py` (missed: 6, FP: 2) — callable protocol assignability

### Constructors (1 file, 12 missed)

- [ ] `constructors_callable.py` (missed: 12) — callable as constructor, `__init_subclass__`

### Dataclasses (1 file, ~9 missed)

- [ ] `dataclasses_transform_converter.py` (missed: 6, FP: 3) — `dataclass_transform` frozen/converter semantics

### TypedDict (2 files, ~17 missed)

- [ ] `typeddicts_readonly_inheritance.py` (missed: 9, FP: 2) — readonly + inheritance rules
- [ ] `typeddicts_type_consistency.py` (missed: 8, FP: 2) — type consistency validation

### TypedDict (1 file — regression)

- [ ] `typeddicts_extra_items.py` (missed: 19, FP: 11) — `extra_items` kwarg, was passing, needs FP cleanup

### Directives (1 file, 3 missed)

- [ ] `directives_version_platform.py` (missed: 3) — dead branch elimination for `sys.version_info`, `sys.platform`

### TypeForm (1 file)

- [ ] `typeforms_typeform.py` — `TypeForm` support (PEP 747)
