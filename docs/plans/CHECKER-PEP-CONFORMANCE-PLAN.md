# PEP Conformance — Plan

> **Score**: 125/146 (85.6%)
> **Tests**: `crates/basilisk-cli/tests/conformance/`
> **Status CSV**: `conformance/conformance_status.csv`
> **Run**: `./conformance/conformance.sh` or `cargo test --test conformance_tests -- --nocapture`

---

## TODO

### Protocols

- [ ] `protocols_definition.py` (missed: 17, FP: 13) — structural subtyping: attrs satisfy properties, method signature compatibility
- [ ] `protocols_generic.py` (missed: 6, FP: 1) — variance tracking in generic protocol type assignability

### Generics — TypeVarTuple

- [ ] `generics_typevartuple_basic.py` (missed: 8) — unpack semantics, TypeVarTuple in generic classes
- [ ] `generics_typevartuple_args.py` (missed: 9) — *args typing with TypeVarTuple
- [ ] `generics_typevartuple_specialization.py` (missed: 3, FP: 2) — TypeVarTuple concrete specialization

### Generics — ParamSpec

- [ ] `generics_paramspec_components.py` (missed: 16) — `P.args`, `P.kwargs` component access
- [ ] `generics_paramspec_semantics.py` (missed: 9, FP: 1) — ParamSpec constraint solving
- [ ] `generics_paramspec_specialization.py` (missed: 5, FP: 3) — ParamSpec concrete specialization

### Generics — Variance

- [ ] `generics_variance_inference.py` (missed: 23, FP: 2) — auto-variance from covariant/contravariant/invariant positions
- [ ] `generics_syntax_infer_variance.py` (missed: 16, FP: 1) — PEP 695 syntax variance inference

### Generics — Defaults

- [ ] `generics_defaults_referential.py` (missed: 2) — TypeVar defaults referencing other TypeVars in generic constructors

### Type Aliases

- [ ] `aliases_type_statement.py` (missed: 10, FP: 4) — PEP 695 `type` statement aliases
- [ ] `aliases_typealiastype.py` (missed: 22, FP: 3) — `TypeAliasType` call-based aliases

### Callables

- [ ] `callables_kwargs.py` (missed: 9, FP: 1) — `**kwargs` type checking in callable signatures
- [ ] `callables_protocol.py` (missed: 6, FP: 2) — callable protocol assignability

### Constructors

- [ ] `constructors_callable.py` (missed: 12) — callable as constructor, `__init_subclass__`

### Dataclasses

- [ ] `dataclasses_transform_converter.py` (missed: 6, FP: 3) — `dataclass_transform` frozen/converter semantics

### TypedDict

- [ ] `typeddicts_readonly_inheritance.py` (missed: 9, FP: 2) — readonly + inheritance rules
- [ ] `typeddicts_type_consistency.py` (missed: 8, FP: 2) — type consistency validation
- [ ] `typeddicts_extra_items.py` (missed: 19, FP: 11) — `extra_items` kwarg, regression, needs FP cleanup

### Directives

- [ ] `directives_version_platform.py` (missed: 3) — dead branch elimination for `sys.version_info`, `sys.platform`

### TypeForm

- [ ] `typeforms_typeform.py` — `TypeForm` support (PEP 747)
