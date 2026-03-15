# PEP Conformance — Plan

> **Score**: 125/146 (85.6%)
> **Tests**: `crates/basilisk-cli/tests/conformance/`
> **Status CSV**: `conformance/conformance_status.csv`
> **Run**: `./conformance/conformance.sh` or `cargo test --test conformance_tests -- --nocapture`

---

## TODO

### Protocols

- [ ] Implement structural subtyping for protocols — attrs satisfy properties, method signature compatibility
- [ ] Add variance tracking in generic protocol type assignability

### Generics — TypeVarTuple

- [ ] Implement unpack semantics and TypeVarTuple in generic classes
- [ ] Support `*args` typing with TypeVarTuple
- [ ] Handle TypeVarTuple concrete specialization

### Generics — ParamSpec

- [ ] Implement `P.args` and `P.kwargs` component access
- [ ] Add ParamSpec constraint solving
- [ ] Handle ParamSpec concrete specialization

### Generics — Variance

- [ ] Infer variance automatically from covariant/contravariant/invariant positions
- [ ] Support PEP 695 syntax variance inference

### Generics — Defaults

- [ ] Support TypeVar defaults referencing other TypeVars in generic constructors

### Type Aliases

- [ ] Implement PEP 695 `type` statement aliases
- [ ] Support `TypeAliasType` call-based aliases

### Callables

- [ ] Type check `**kwargs` in callable signatures
- [ ] Validate callable protocol assignability

### Constructors

- [ ] Support callable as constructor and `__init_subclass__`

### Dataclasses

- [ ] Implement `dataclass_transform` frozen/converter semantics

### TypedDict

- [ ] Enforce readonly + inheritance rules for TypedDict
- [ ] Validate TypedDict type consistency
- [ ] Support `extra_items` kwarg in TypedDict

### Directives

- [ ] Implement dead branch elimination for `sys.version_info` and `sys.platform`

### TypeForm

- [ ] Implement `TypeForm` support (PEP 747)
