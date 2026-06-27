# Type Narrowing and Full Inference — Plan {#NARROWPLAN}

> **Spec**: [CHECKER-TYPE-INFERENCE-SPEC.md §TYPEINF-OVERVIEW](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-OVERVIEW)
> **Motivation**: [CHECK-ELIMINATE-FALSE-POSITIVES.md](CHECK-ELIMINATE-FALSE-POSITIVES.md) — ~125 FPs blocked on this work
> **Architecture**: [CHECKER-ARCHITECTURE-SPEC.md §CHKARCH-INFERENCE](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INFERENCE)

---

## Context {#NARROWPLAN-CONTEXT}

The checker currently uses text-based annotation parsing and literal-only RHS inference. It has no control-flow graph, no type narrowing, and no TypeVar constraint solving. ~125 of the remaining ~196 false positives in the conformance suite cannot be fixed without this fundamental engine work.

The spec ([CHECKER-TYPE-INFERENCE-SPEC.md §TYPEINF-OVERVIEW](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-OVERVIEW)) defines the full target state. This plan is the implementation roadmap to get there.

### Current State {#NARROWPLAN-CONTEXT-CURRENT-STATE}

| Component | Spec Reference | Status |
|-----------|---------------|--------|
| `InferredType` enum (47 variants) | [§TYPEINF-INFERRED](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-INFERRED) | Implemented |
| `is_assignable_to()` | [§TYPEINF-SUBTYPING](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-SUBTYPING) | Implemented (covariance, contravariance, numeric widening) |
| `from_annotation()` text parser | [§TYPEINF-VARS-ANNOTATED](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-VARS-ANNOTATED) | Implemented — falls back to `Named(String)` for complex types |
| `infer_rhs()` | [§TYPEINF-VARS-SIMPLE](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-VARS-SIMPLE) | Literal-only — skips function calls, attribute access, subscripts |
| `FlowUnionTracker` | [§TYPEINF-VARS-FLOW](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-VARS-FLOW) | Basic implementation, unused by any rule |
| `NarrowingEngine` | [§TYPEINF-NARROWING](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING) | **Stub** — `narrowing.rs` is 2 lines |
| `ConstraintSolver` | [§TYPEINF-GENERICS-TYPEVAR](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-GENERICS-TYPEVAR) | **Not implemented** |
| `OverloadResolver` | [§TYPEINF-FUNC-OVERLOADS](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-FUNC-OVERLOADS) | **Not implemented** |

### What This Unblocks {#NARROWPLAN-CONTEXT-UNBLOCKS}

**assignment_compatibility (~98 FP)**: Assignment checks compare annotation text to RHS literal kind. When the RHS is a function call, parameter reference, class instantiation, or any non-literal expression, the checker cannot determine the RHS type. Named-to-Named subtyping (e.g., `x: Animal = Dog()`) requires resolving class hierarchies and protocol structural conformance.

**directives_assert_type_2 (~12 FP)**: `assert_type()` validation is deliberately disabled (comment in source: "requires full type inference to avoid false positives"). Re-enabling requires knowing the inferred type at every expression site, including after narrowing guards.

**returns_compatibility_2 (~15 FP)**: Return type checking skips function call RHS entirely. Protocol property return types, narrowing function return types, and context manager `__exit__` return types all require resolving call targets.

---

## Phase 1: NarrowingEngine {#NARROWPLAN-ENGINE}

**File**: `crates/basilisk-checker/src/narrowing.rs` (expand stub)
**Spec**: [§TYPEINF-NARROWING-ISINSTANCE](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-ISINSTANCE) through [§TYPEINF-NARROWING-SCOPE](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-SCOPE)

Build a per-function narrowing engine that tracks type state through control flow.

### 1a. Core Data Structure {#NARROWPLAN-ENGINE-DATA-STRUCTURE}

```rust
struct NarrowingContext {
    /// Variable name -> narrowed type at this point in control flow
    type_state: HashMap<String, InferredType>,
    /// Stack of saved states for branch/join points
    branch_stack: Vec<HashMap<String, InferredType>>,
}

impl NarrowingContext {
    /// Fork state at a branch point (if/else, match/case)
    fn push_branch(&mut self);
    /// Join state from two branches (union of types)
    fn pop_and_join(&mut self);
    /// Narrow a variable's type within the current branch
    fn narrow(&mut self, var: &str, narrowed: InferredType);
    /// Query the current narrowed type of a variable
    fn get_type(&self, var: &str) -> Option<&InferredType>;
}
```

### 1b. Narrowing Patterns (in priority order) {#NARROWPLAN-ENGINE-PATTERNS}

1. **isinstance narrowing** ([§TYPEINF-NARROWING-ISINSTANCE](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-ISINSTANCE)) — `isinstance(x, T)` narrows `x` to `T` in `if`, complement in `else`
2. **None narrowing** ([§TYPEINF-NARROWING-NONE](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-NONE)) — `x is None` / `x is not None`
3. **Truthiness narrowing** ([§TYPEINF-NARROWING-TRUTHY](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-TRUTHY)) — `if x:` removes falsy types (`None`, `Literal[0]`, `Literal[""]`, `Literal[False]`)
4. **Assignment narrowing** ([§TYPEINF-NARROWING-ASSIGN](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-ASSIGN)) — `x = 42` narrows `x: int | str` to `int`
5. **Assert narrowing** ([§TYPEINF-NARROWING-ASSERT](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-ASSERT)) — `assert x is not None` narrows for all subsequent code
6. **TypeGuard narrowing** ([§TYPEINF-NARROWING-TYPEGUARD](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-TYPEGUARD)) — positive branch only (per PEP 647)
7. **TypeIs narrowing** ([§TYPEINF-NARROWING-TYPEIS](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-TYPEIS)) — bidirectional, both branches (per PEP 742)
8. **Pattern match narrowing** ([§TYPEINF-NARROWING-MATCH](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-MATCH)) — exhaustiveness checking

### 1c. Approach {#NARROWPLAN-ENGINE-APPROACH}

Walk the function body as a sequence of narrowing events. At branch points (`if`/`else`, `match`/`case`), fork the type state. At join points, union the states. This does not require a full CFG — a structured walk of the AST is sufficient for Python's block-scoped control flow.

### 1d. Scope Limitations ([§TYPEINF-NARROWING-SCOPE](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-SCOPE)) {#NARROWPLAN-ENGINE-SCOPE}

Narrowing does NOT persist across:
- Function boundaries (inner functions capture the unnarrowed type)
- Loop bodies (narrowed type before loop resets at each iteration)
- After reassignment of the narrowed variable

### 1e. Verification {#NARROWPLAN-ENGINE-VERIFICATION}

- Re-enable E0053 `assert_type()` after narrowing guards
- Conformance files: `narrowing_typeguard.py`, `narrowing_typeis.py`
- Expected: ~15 FPs fixed

---

## Phase 2: Expression Type Inference {#NARROWPLAN-EXPR-INFERENCE}

**File**: `crates/basilisk-checker/src/inference.rs` (expand existing)
**Spec**: [§TYPEINF-SUBTYPING](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-SUBTYPING), [§TYPEINF-BIDIR](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-BIDIR)

Extend `infer_rhs()` from literal-only to full expression inference.

### 2a. Function Call Return Types (highest value) {#NARROWPLAN-EXPR-INFERENCE-CALLS}

Resolve call target → function signature → return type annotation:
- Same-module functions: look up in `ResolvedModule::function_defs`
- Cross-module functions: look up via `ImportGraph` + target `ResolvedModule`
- Constructor calls: `ClassName()` → `ClassName` (the class type itself)
- Method calls: `obj.method()` → resolve `method` on class → return type

This is the single highest-value target — it unblocks the majority of E0014 FPs where the RHS is `Dog()`, `create_widget()`, etc.

### 2b. Attribute Access {#NARROWPLAN-EXPR-INFERENCE-ATTRIBUTE}

`x.attr` resolves via:
- Class definition → attribute annotation type
- `@property` → return type of the getter
- Module-level → variable annotation type

### 2c. Subscript {#NARROWPLAN-EXPR-INFERENCE-SUBSCRIPT}

`x[key]` resolves via:
- `list[T].__getitem__` → `T`
- `dict[K, V].__getitem__` → `V`
- TypedDict field access → field type
- `tuple[A, B, C].__getitem__` with literal index → element type

### 2d. Binary and Unary Operations {#NARROWPLAN-EXPR-INFERENCE-OPERATORS}

`a + b` resolves via `__add__` return type. For builtins, use a hardcoded table:
- `int + int` → `int`, `int + float` → `float`, `str + str` → `str`
- `not x` → `bool`, `-x` on `int` → `int`

### 2e. Conditional Expression {#NARROWPLAN-EXPR-INFERENCE-CONDITIONAL}

`a if cond else b` → `Union[type(a), type(b)]`

### 2f. Walrus Operator {#NARROWPLAN-EXPR-INFERENCE-WALRUS}

`(x := expr)` has the type of `expr` ([§TYPEINF-VARS-WALRUS](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-VARS-WALRUS))

### 2g. Verification {#NARROWPLAN-EXPR-INFERENCE-VERIFICATION}

- E0014: function call assignments should stop being flagged as FPs
- E0013: return statements with function calls should be type-checked
- Expected: ~40 FPs fixed

---

## Phase 3: ConstraintSolver — TypeVar Resolution {#NARROWPLAN-SOLVER}

**File**: `crates/basilisk-checker/src/constraint_solver.rs` (new)
**Spec**: [§TYPEINF-GENERICS-TYPEVAR](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-GENERICS-TYPEVAR) through [§TYPEINF-GENERICS-DEFAULTS](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-GENERICS-DEFAULTS)

### 3a. Algorithm {#NARROWPLAN-SOLVER-ALGORITHM}

1. Collect constraints from argument types against TypeVar-bearing parameter types
2. Compute the join (union) of lower-bound constraints
3. Use expected return type as additional bidirectional constraint
4. Solve constrained TypeVars by matching against constraint set ([§TYPEINF-GENERICS-CONSTRAINED](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-GENERICS-CONSTRAINED))
5. Handle bound TypeVars — upper bound check ([§TYPEINF-GENERICS-BOUND](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-GENERICS-BOUND))
6. Handle TypeVar defaults ([§TYPEINF-GENERICS-DEFAULTS](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-GENERICS-DEFAULTS), PEP 696)

### 3b. Key Interface {#NARROWPLAN-SOLVER-INTERFACE}

```rust
struct ConstraintSolver {
    constraints: HashMap<String, Vec<TypeConstraint>>,
}

enum TypeConstraint {
    /// T must be a supertype of this
    LowerBound(InferredType),
    /// T must be a subtype of this
    UpperBound(InferredType),
    /// T must be exactly one of these (constrained TypeVar)
    OneOf(Vec<InferredType>),
}

impl ConstraintSolver {
    fn add_constraint(&mut self, typevar: &str, constraint: TypeConstraint);
    fn solve(&self) -> Result<HashMap<String, InferredType>, SolveError>;
}
```

### 3c. Deferred {#NARROWPLAN-SOLVER-DEFERRED}

- ParamSpec solving ([§TYPEINF-GENERICS-PARAMSPEC](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-GENERICS-PARAMSPEC)) — address in follow-up
- TypeVarTuple solving — address in follow-up

### 3d. Verification {#NARROWPLAN-SOLVER-VERIFICATION}

- Generic function calls: `first([1, 2, 3])` should resolve `T = int`
- Conformance files: `generics_basic.py`, `generics_defaults.py`
- Expected: ~10 FPs fixed

---

## Phase 4: Class Hierarchy and Structural Subtyping {#NARROWPLAN-SUBTYPING}

**File**: `crates/basilisk-checker/src/subtyping.rs` (new)
**Spec**: [CHECKER-TYPE-INFERENCE-SPEC.md §TYPEINF-SUBTYPING](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-SUBTYPING)

This is the **single largest FP blocker** (~80 of the remaining 177 FPs). The current `is_assignable_to()` in `types.rs` compares `Named` types by string name — it cannot determine that `Dog` is a subtype of `Animal` or that a class with `def draw(self)` satisfies a `Drawable` Protocol.

### 4a. `SubtypeContext` — Core Data Structure {#NARROWPLAN-SUBTYPING-DATA-STRUCTURE}

```rust
struct SubtypeContext<'a> {
    /// Class name → ordered MRO (C3 linearization)
    mro_cache: HashMap<&'a str, Vec<&'a str>>,
    /// Class name → ClassInfo (from ResolvedModule)
    class_map: HashMap<&'a str, &'a ClassInfo>,
    /// Class name → method signatures
    method_map: HashMap<(&'a str, &'a str), &'a FunctionInfo>,
    /// Protocol name → required members (name, kind, type)
    protocol_members: HashMap<&'a str, Vec<ProtocolMember<'a>>>,
}

struct ProtocolMember<'a> {
    name: &'a str,
    kind: MemberKind,  // Method, Property, ReadWriteProperty, Attribute
    type_sig: Option<&'a str>,  // Return type for methods/properties, type for attributes
}
```

### 4b. Nominal Subtyping via MRO {#NARROWPLAN-SUBTYPING-NOMINAL}

**Algorithm**: Given source class `S` and target class `T`:
1. Look up `S` in `mro_cache`. If not cached, compute C3 linearization from `ClassInfo.bases`.
2. Check if `T` appears anywhere in `S`'s MRO.
3. If yes → subtype. If no → not a nominal subtype (may still be structural).

**Builtin MRO** (hardcoded for types not defined in user code):
- `bool` → `[bool, int, float, complex, object]`
- `int` → `[int, float, complex, object]`
- `list` → `[list, MutableSequence, Sequence, Reversible, Collection, Iterable, object]`
- `dict` → `[dict, MutableMapping, Mapping, Collection, Iterable, object]`
- `str` → `[str, Sequence, Hashable, object]`
- `tuple` → `[tuple, Sequence, Hashable, object]`
- `set` → `[set, MutableSet, AbstractSet, Collection, Iterable, object]`
- `frozenset` → `[frozenset, AbstractSet, Collection, Iterable, object]`

### 4c. Protocol Structural Subtyping (PEP 544) {#NARROWPLAN-SUBTYPING-PROTOCOL}

**Algorithm**: Given source class `S` and protocol `P`:

1. **Collect `P`'s members**: walk `P`'s class body + Protocol bases (NOT `object` or `Generic`). For each:
   - `def method(self, ...) -> R` → `ProtocolMember { kind: Method, type_sig: signature }`
   - `@property def prop(self) -> R` → `ProtocolMember { kind: Property, type_sig: R }`
   - `attr: T` → `ProtocolMember { kind: Attribute, type_sig: T }`

2. **For each protocol member**, search `S` for a matching member:
   - Check `S.methods` (name match → compare signatures with [§TYPEINF-SUBTYPING-CALLABLE](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-SUBTYPING-CALLABLE) callable subtyping)
   - Check `S.attributes` (name match → check type compatibility)
   - Check dataclass fields, `NamedTuple` fields (name match → attribute equivalent)
   - Walk `S`'s MRO for inherited members

3. **If ALL members matched** → `S` satisfies `P`. Otherwise → not a subtype.

**Key subtlety**: A plain attribute `x: int` satisfies a `@property` requirement `def x(self) -> int`. A mutable attribute satisfies a read-write property. An immutable attribute (or `@property` without setter) does NOT satisfy a read-write property.

### 4d. Generic Subtyping with Variance {#NARROWPLAN-SUBTYPING-GENERIC}

**Algorithm**: Given `Source[A1, A2]` and `Target[B1, B2]`:

1. Check that `Source`'s base class (or MRO) includes `Target`'s base.
2. Determine the TypeVar mapping: how `Source` specializes `Target`.
3. For each TypeVar position:
   - **Covariant**: `A_i` must be a subtype of `B_i`
   - **Contravariant**: `B_i` must be a subtype of `A_i`
   - **Invariant**: `A_i` must equal `B_i` (bidirectional subtype)

### 4e. TypedDict Structural Subtyping {#NARROWPLAN-SUBTYPING-TYPEDDICT}

**Algorithm**: Given TypedDict `S` assigned to TypedDict `T`:

1. For each required field `f: U` in `T`:
   - `S` must have field `f` with type `V` where `V` <: `U` (for `ReadOnly` fields) or `V` == `U` (for mutable fields)
2. For each `NotRequired` field `f: U` in `T`:
   - If `S` has field `f`, its type must be compatible as above
   - If `S` lacks field `f`, that's OK (it's not required)
3. If `T` has `extra_items=U`:
   - Any field in `S` not declared in `T` must have type `V` <: `U`
4. If `T` does NOT have `extra_items`:
   - `S` must not have fields that `T` doesn't declare (closed schema)

### 4f. Callable Subtyping ([§TYPEINF-SUBTYPING-CALLABLE](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-SUBTYPING-CALLABLE)) {#NARROWPLAN-SUBTYPING-CALLABLE}

Already partially implemented in `is_assignable_to()`. Needs extension for:
- `*args`/`**kwargs` parameter compatibility
- Default parameter arity tolerance (source can have more defaults than target)
- `Protocol.__call__` ↔ `Callable` equivalence
- `Concatenate[X, P]` parameter prepending

### 4g. Wire Into `is_subtype_of()` {#NARROWPLAN-SUBTYPING-WIRE}

Replace the current `is_assignable_to()` fallback:

```rust
// Current: Named-to-Named → compare base names (wrong for hierarchies)
(InferredType::Named(a), InferredType::Named(b)) => a_base == b_base

// New: dispatch to SubtypeContext
(InferredType::Named(a), InferredType::Named(b)) =>
    ctx.is_subtype(a, b)  // MRO lookup + protocol check + generic variance
```

### 4h. Verification {#NARROWPLAN-SUBTYPING-VERIFICATION}

- E0014: `x: Animal = Dog()` should pass
- E0014: `f: Standard2 = pos_only  # E` should still fail (not structurally compatible)
- Conformance files: `protocols_subtyping.py`, `callables_subtyping.py`, `protocols_definition.py`
- Expected: ~50 FPs fixed

---

## Phase 5: Wire Into Rules {#NARROWPLAN-WIRE-RULES}

### 5a. E0014 (Assignment Type Mismatch) {#NARROWPLAN-WIRE-RULES-E0014}

Replace:
- `infer_rhs()` → `inference_engine.infer_expr(rhs)`
- `from_annotation()` → `inference_engine.resolve_type(annotation)`
- `is_assignable_to()` → `subtyping.is_subtype_of()`

### 5b. E0013 (Return Type Mismatch) {#NARROWPLAN-WIRE-RULES-E0013}

- Infer return expression types including function calls
- Use narrowing context for narrowed return values
- Stop skipping call expressions

### 5c. E0053 (assert_type) {#NARROWPLAN-WIRE-RULES-E0053}

- Re-enable the rule (currently disabled with comment)
- Use `inference_engine.infer_expr()` at the call site
- Compare inferred type to expected type argument

### 5d. Verification {#NARROWPLAN-WIRE-RULES-VERIFICATION}

- Run full conformance suite
- Expected: ~10 additional FPs fixed from edge cases
- Total Phase 2 target: ~125 FPs eliminated

---

## Execution Order {#NARROWPLAN-EXECUTION-ORDER}

| Phase | Unblocks | Effort | Est. FPs Fixed | Dependencies |
|-------|----------|--------|----------------|--------------|
| 1. NarrowingEngine | E0053, E0013 narrowing | High | ~15 | None |
| 2. Expression inference | E0014 calls, E0013 calls | High | ~40 | None |
| 3. ConstraintSolver | Generic function calls | High | ~10 | Phase 2 |
| 4. Class hierarchy + structural subtyping | Named-to-Named, protocols, TypedDict, callables | High | ~50 | Phase 2 |
| 5. Wire into rules | All remaining | Medium | ~10 | Phases 1-4 |

Phases 1 and 2 are independent and can be parallelized. Phase 3 depends on Phase 2 (needs expression inference to collect constraints). Phase 4 depends on Phase 2 (needs resolved types). Phase 5 depends on all prior phases.

---

## Risks and Mitigations {#NARROWPLAN-RISKS}

| Risk | Mitigation |
|------|------------|
| Expression inference introduces false negatives (misses real errors) | Gate behind feature flag initially; run conformance suite with flag on/off; missed count must not increase |
| Narrowing state explosion in deeply nested control flow | Cap branch stack depth; fall back to unnarrowed type beyond limit |
| MRO resolution is expensive for deep hierarchies | Cache per class in `ResolvedModule`; invalidate on file change via Salsa |
| ConstraintSolver doesn't converge for complex generic chains | Bound recursion depth; emit diagnostic on unsolvable TypeVars rather than crash |
| Cross-module type resolution adds latency | Keep same-module fast path; cross-module resolution is already cached by import graph |

---

## TODO {#NARROWPLAN-TODO}

- [ ] **Phase 1: NarrowingEngine** (~15 FPs)
  - [ ] 1a. `NarrowingContext` data structure with push/pop/join — `crates/basilisk-checker/src/narrowing.rs`
  - [ ] 1b. isinstance narrowing ([§TYPEINF-NARROWING-ISINSTANCE](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-ISINSTANCE)) — positive + complement branches
  - [ ] 1c. None narrowing ([§TYPEINF-NARROWING-NONE](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-NONE)) — `is None` / `is not None` with `remove_none()`
  - [ ] 1d. Truthiness narrowing ([§TYPEINF-NARROWING-TRUTHY](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-TRUTHY)) — `remove_falsy()` / `keep_falsy()`
  - [ ] 1e. Assignment narrowing ([§TYPEINF-NARROWING-ASSIGN](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-ASSIGN)) — narrows to assigned type
  - [ ] 1f. Assert narrowing ([§TYPEINF-NARROWING-ASSERT](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-ASSERT)) — unwraps inner guard, applies unconditionally
  - [ ] 1g. TypeGuard narrowing — positive branch only ([§TYPEINF-NARROWING-TYPEGUARD](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-TYPEGUARD))
  - [ ] 1h. TypeIs narrowing — bidirectional ([§TYPEINF-NARROWING-TYPEIS](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-TYPEIS)) — positive + complement
  - [ ] 1i. Pattern match narrowing + exhaustiveness ([§TYPEINF-NARROWING-MATCH](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-MATCH)) — per-case + wildcard detection
  - [ ] 1j. Scope limitations ([§TYPEINF-NARROWING-SCOPE](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-SCOPE)) — `in_loop` flag, no recursion into nested functions
  - [x] Resolver guard collection — `crates/basilisk-resolver/src/visitor/narrowing.rs` + `narrowing_types.rs`
  - [x] `FunctionInfo.narrowing_guards` field wired into `function_info_from()`
- [ ] **Phase 2: Expression Type Inference** (~40 FPs) — `crates/basilisk-checker/src/expr_inference.rs`
  - [ ] 2a. Function call return type resolution (same-module) — `ExpressionInferrer::resolve_call_return_type()`
  - [ ] 2b. Constructor call resolution (`ClassName()` → class type) — via `class_names` lookup
  - [ ] 2c. Cross-module function call return types — via `imported_symbols` + `type_annotation`
  - [ ] 2d. Method call resolution (`obj.method()`) — `resolve_method_return_type()`
  - [ ] 2e. Attribute access type resolution — `resolve_attribute_type()` via class `annotation_span`
  - [ ] 2f. Subscript type resolution (list/dict/TypedDict/tuple) — `resolve_subscript_type()`
  - [ ] 2g. Binary/unary operation return types — `infer_binop_type()` / `infer_unaryop_type()` with builtin tables
  - [ ] 2h. Conditional expression (`a if cond else b` → union) — `infer_conditional_type()`
  - [ ] 2i. Walrus operator type propagation — pass-through (caller passes expr type)
  - [ ] Builtin constructor table — 40+ builtins (`int`, `str`, `len`, `sorted`, `open`, etc.)
  - [ ] Builtin method table — `str`, `list`, `dict`, `set`, `int`, `float`, `bytes`, `tuple` methods
- [ ] **Phase 3: ConstraintSolver** (~10 FPs) — `crates/basilisk-checker/src/constraint_solver.rs`
  - [ ] 3a. Constraint collection — `add_lower_bound()`, `add_upper_bound()`, `add_one_of()`
  - [ ] 3b. Lower-bound join / upper-bound meet solving — `solve()` + `solve_one()`
  - [ ] 3c. Bidirectional constraint from expected return type — `add_return_constraint()`
  - [ ] 3d. Constrained TypeVar matching ([§TYPEINF-GENERICS-CONSTRAINED](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-GENERICS-CONSTRAINED)) — `solve_constrained()` with widening
  - [ ] 3e. Bound TypeVar upper-bound check ([§TYPEINF-GENERICS-BOUND](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-GENERICS-BOUND)) — validates `is_assignable_to(bound)`
  - [ ] 3f. TypeVar defaults (PEP 696, [§TYPEINF-GENERICS-DEFAULTS](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-GENERICS-DEFAULTS)) — `set_default()` + fallback in `solve()`
- [ ] **Phase 4: Class Hierarchy and Structural Subtyping** (~50 FPs) — `crates/basilisk-checker/src/subtyping.rs`
  - [ ] 4a. `SubtypeContext` data structure with MRO cache, protocol member tables — `SubtypeContext::from_module()`
  - [ ] 4b. Nominal subtyping via C3 MRO resolution ([§TYPEINF-SUBTYPING-NOMINAL](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-SUBTYPING-NOMINAL)) + builtin MRO hardcoding — `compute_mro()` + `builtin_mro()`
  - [ ] 4c. Protocol structural subtyping: member collection, method/attribute/property matching ([§TYPEINF-SUBTYPING-PROTOCOL](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-SUBTYPING-PROTOCOL)) — `is_protocol_subtype()` + `source_has_member()`
  - [ ] 4d. Generic subtyping with variance-aware TypeVar position checking ([§TYPEINF-SUBTYPING-GENERIC](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-SUBTYPING-GENERIC)) — `is_subtype_with_context()` Callable contravariance
  - [ ] 4e. TypedDict structural subtyping: Required/NotRequired/ReadOnly/extra_items ([§TYPEINF-SUBTYPING-TYPEDDICT](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-SUBTYPING-TYPEDDICT)) — `is_typeddict_subtype()` + `parse_typeddict_field_flags()`
  - [ ] 4f. Callable subtyping: contravariant params, covariant return, ellipsis ([§TYPEINF-SUBTYPING-CALLABLE](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-SUBTYPING-CALLABLE)) — `is_subtype_with_context()` Callable arm
  - [ ] 4g. Wire `is_subtype_of()` to replace Named-to-Named string comparison — `is_subtype_with_context()` dispatches Named→SubtypeContext
  - [ ] 4h. Conformance verification — 57 FPs (under 71 target), 0 regressions
- [ ] **Phase 5: Wire Into Rules** (~10 FPs)
  - [ ] 5a. E0014 — `VarCheckContext` with `SubtypeContext`, uses `is_subtype_with_context()` for assignability
  - [ ] 5b. E0013 — `SubtypeContext` passed to `check_function()`, removed `contains_named` early exit for Named types
  - [ ] 5c. E0053 — `is_likely_narrowed()` heuristic suppresses narrowing-dependent FPs; Union normalization in `types_match()`
  - [x] 5d. Full conformance suite verification — the official, UNMODIFIED `python/typing` scorer (pinned `268d0c4e`) reports **68 of 146 fixtures passing (46.6%)** with the `basilisk` binary run with **EVERY rule enabled** — no config, no `basilisk.json`, no "spec-conformance mode", no exceptions. That score reflects **265 false positives and 0 missed required errors**: the checker catches every required error, and every failing fixture is false positives from strict-by-default house-style rules (require-annotation E0001/E0002/E0004, missing-@override E0025, explicit-Any W0014, redundant-annotation W0050) firing on spec-valid code that the spec treats as inferred rather than an error. HISTORY: the last honest score was 59/146 = 40.4% (285 FPs) at PR #183; PRs #184/#185/#191 inflated the reported number to a fake 100% by writing a `basilisk.json` that DISABLED those 6 house rules at score time — the checker was never made smarter, the FPs were merely hidden. That disabling has been removed, and disabling any conformance rule for scoring is now forbidden (a punishable offence). Genuine progress over that span was real but modest: 40.4% → 46.6%. The only legitimate path to 100% is fixing the checker so its strict defaults stop firing on spec-valid code, with every rule still enabled — never by disabling a rule. Driving FPs down remains active work.
  - [ ] Checker-side modules: `narrowing.rs` (NarrowingContext), `expr_inference.rs` (ExpressionInferrer), `constraint_solver.rs` (ConstraintSolver)
