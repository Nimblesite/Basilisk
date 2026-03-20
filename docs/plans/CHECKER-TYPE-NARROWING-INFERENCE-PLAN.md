# Type Narrowing and Full Inference — Plan

> **Spec**: [CHECKER-TYPE-INFERENCE-SPEC.md](../specs/CHECKER-TYPE-INFERENCE-SPEC.md)
> **Motivation**: [CHECK-ELIMINATE-FALSE-POSITIVES.md](CHECK-ELIMINATE-FALSE-POSITIVES.md) — ~125 FPs blocked on this work
> **Architecture**: [CHECKER-ARCHITECTURE-SPEC.md](../specs/CHECKER-ARCHITECTURE-SPEC.md)

---

## Context

The checker currently uses text-based annotation parsing and literal-only RHS inference. It has no control-flow graph, no type narrowing, and no TypeVar constraint solving. ~125 of the remaining ~196 false positives in the conformance suite cannot be fixed without this fundamental engine work.

The spec (`CHECKER-TYPE-INFERENCE-SPEC.md`) defines the full target state. This plan is the implementation roadmap to get there.

### Current State

| Component | Spec Reference | Status |
|-----------|---------------|--------|
| `InferredType` enum (47 variants) | §2.1 | Implemented |
| `is_assignable_to()` | §2.3 | Implemented (covariance, contravariance, numeric widening) |
| `from_annotation()` text parser | §3.3 | Implemented — falls back to `Named(String)` for complex types |
| `infer_rhs()` | §3.1 | Literal-only — skips function calls, attribute access, subscripts |
| `FlowUnionTracker` | §3.2 | Basic implementation, unused by any rule |
| `NarrowingEngine` | §7 | **Stub** — `narrowing.rs` is 2 lines |
| `ConstraintSolver` | §6.1 | **Not implemented** |
| `OverloadResolver` | §4.6 | **Not implemented** |

### What This Unblocks

**BSK-E0014 (~98 FP)**: Assignment checks compare annotation text to RHS literal kind. When the RHS is a function call, parameter reference, class instantiation, or any non-literal expression, the checker cannot determine the RHS type. Named-to-Named subtyping (e.g., `x: Animal = Dog()`) requires resolving class hierarchies and protocol structural conformance.

**BSK-E0053 (~12 FP)**: `assert_type()` validation is deliberately disabled (comment in source: "requires full type inference to avoid false positives"). Re-enabling requires knowing the inferred type at every expression site, including after narrowing guards.

**BSK-E0013 (~15 FP)**: Return type checking skips function call RHS entirely. Protocol property return types, narrowing function return types, and context manager `__exit__` return types all require resolving call targets.

---

## Phase 1: NarrowingEngine

**File**: `crates/basilisk-checker/src/narrowing.rs` (expand stub)
**Spec**: §7.1–7.10

Build a per-function narrowing engine that tracks type state through control flow.

### 1a. Core Data Structure

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

### 1b. Narrowing Patterns (in priority order)

1. **isinstance narrowing** (§7.1) — `isinstance(x, T)` narrows `x` to `T` in `if`, complement in `else`
2. **None narrowing** (§7.2) — `x is None` / `x is not None`
3. **Truthiness narrowing** (§7.3) — `if x:` removes falsy types (`None`, `Literal[0]`, `Literal[""]`, `Literal[False]`)
4. **Assignment narrowing** (§7.4) — `x = 42` narrows `x: int | str` to `int`
5. **Assert narrowing** (§7.8) — `assert x is not None` narrows for all subsequent code
6. **TypeGuard narrowing** (§7.6) — positive branch only (per PEP 647)
7. **TypeIs narrowing** (§7.7) — bidirectional, both branches (per PEP 742)
8. **Pattern match narrowing** (§7.5) — exhaustiveness checking

### 1c. Approach

Walk the function body as a sequence of narrowing events. At branch points (`if`/`else`, `match`/`case`), fork the type state. At join points, union the states. This does not require a full CFG — a structured walk of the AST is sufficient for Python's block-scoped control flow.

### 1d. Scope Limitations (§7.10)

Narrowing does NOT persist across:
- Function boundaries (inner functions capture the unnarrowed type)
- Loop bodies (narrowed type before loop resets at each iteration)
- After reassignment of the narrowed variable

### 1e. Verification

- Re-enable E0053 `assert_type()` after narrowing guards
- Conformance files: `narrowing_typeguard.py`, `narrowing_typeis.py`
- Expected: ~15 FPs fixed

---

## Phase 2: Expression Type Inference

**File**: `crates/basilisk-checker/src/inference.rs` (expand existing)
**Spec**: §2.3, §8

Extend `infer_rhs()` from literal-only to full expression inference.

### 2a. Function Call Return Types (highest value)

Resolve call target → function signature → return type annotation:
- Same-module functions: look up in `ResolvedModule::function_defs`
- Cross-module functions: look up via `ImportGraph` + target `ResolvedModule`
- Constructor calls: `ClassName()` → `ClassName` (the class type itself)
- Method calls: `obj.method()` → resolve `method` on class → return type

This is the single highest-value target — it unblocks the majority of E0014 FPs where the RHS is `Dog()`, `create_widget()`, etc.

### 2b. Attribute Access

`x.attr` resolves via:
- Class definition → attribute annotation type
- `@property` → return type of the getter
- Module-level → variable annotation type

### 2c. Subscript

`x[key]` resolves via:
- `list[T].__getitem__` → `T`
- `dict[K, V].__getitem__` → `V`
- TypedDict field access → field type
- `tuple[A, B, C].__getitem__` with literal index → element type

### 2d. Binary and Unary Operations

`a + b` resolves via `__add__` return type. For builtins, use a hardcoded table:
- `int + int` → `int`, `int + float` → `float`, `str + str` → `str`
- `not x` → `bool`, `-x` on `int` → `int`

### 2e. Conditional Expression

`a if cond else b` → `Union[type(a), type(b)]`

### 2f. Walrus Operator

`(x := expr)` has the type of `expr` (§3.5)

### 2g. Verification

- E0014: function call assignments should stop being flagged as FPs
- E0013: return statements with function calls should be type-checked
- Expected: ~40 FPs fixed

---

## Phase 3: ConstraintSolver — TypeVar Resolution

**File**: `crates/basilisk-checker/src/constraint_solver.rs` (new)
**Spec**: §6.1–6.5

### 3a. Algorithm

1. Collect constraints from argument types against TypeVar-bearing parameter types
2. Compute the join (union) of lower-bound constraints
3. Use expected return type as additional bidirectional constraint
4. Solve constrained TypeVars by matching against constraint set (§6.2)
5. Handle bound TypeVars — upper bound check (§6.3)
6. Handle TypeVar defaults (§6.5, PEP 696)

### 3b. Key Interface

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

### 3c. Deferred

- ParamSpec solving (§6.6) — address in follow-up
- TypeVarTuple solving — address in follow-up

### 3d. Verification

- Generic function calls: `first([1, 2, 3])` should resolve `T = int`
- Conformance files: `generics_basic.py`, `generics_defaults.py`
- Expected: ~10 FPs fixed

---

## Phase 4: Class Hierarchy and Structural Subtyping

**File**: `crates/basilisk-checker/src/subtyping.rs` (new)
**Spec**: `CHECKER-TYPE-INFERENCE-SPEC.md` §9

This is the **single largest FP blocker** (~80 of the remaining 177 FPs). The current `is_assignable_to()` in `types.rs` compares `Named` types by string name — it cannot determine that `Dog` is a subtype of `Animal` or that a class with `def draw(self)` satisfies a `Drawable` Protocol.

### 4a. `SubtypeContext` — Core Data Structure

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

### 4b. Nominal Subtyping via MRO

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

### 4c. Protocol Structural Subtyping (PEP 544)

**Algorithm**: Given source class `S` and protocol `P`:

1. **Collect `P`'s members**: walk `P`'s class body + Protocol bases (NOT `object` or `Generic`). For each:
   - `def method(self, ...) -> R` → `ProtocolMember { kind: Method, type_sig: signature }`
   - `@property def prop(self) -> R` → `ProtocolMember { kind: Property, type_sig: R }`
   - `attr: T` → `ProtocolMember { kind: Attribute, type_sig: T }`

2. **For each protocol member**, search `S` for a matching member:
   - Check `S.methods` (name match → compare signatures with §9.6 callable subtyping)
   - Check `S.attributes` (name match → check type compatibility)
   - Check dataclass fields, `NamedTuple` fields (name match → attribute equivalent)
   - Walk `S`'s MRO for inherited members

3. **If ALL members matched** → `S` satisfies `P`. Otherwise → not a subtype.

**Key subtlety**: A plain attribute `x: int` satisfies a `@property` requirement `def x(self) -> int`. A mutable attribute satisfies a read-write property. An immutable attribute (or `@property` without setter) does NOT satisfy a read-write property.

### 4d. Generic Subtyping with Variance

**Algorithm**: Given `Source[A1, A2]` and `Target[B1, B2]`:

1. Check that `Source`'s base class (or MRO) includes `Target`'s base.
2. Determine the TypeVar mapping: how `Source` specializes `Target`.
3. For each TypeVar position:
   - **Covariant**: `A_i` must be a subtype of `B_i`
   - **Contravariant**: `B_i` must be a subtype of `A_i`
   - **Invariant**: `A_i` must equal `B_i` (bidirectional subtype)

### 4e. TypedDict Structural Subtyping

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

### 4f. Callable Subtyping (§9.6)

Already partially implemented in `is_assignable_to()`. Needs extension for:
- `*args`/`**kwargs` parameter compatibility
- Default parameter arity tolerance (source can have more defaults than target)
- `Protocol.__call__` ↔ `Callable` equivalence
- `Concatenate[X, P]` parameter prepending

### 4g. Wire Into `is_subtype_of()`

Replace the current `is_assignable_to()` fallback:

```rust
// Current: Named-to-Named → compare base names (wrong for hierarchies)
(InferredType::Named(a), InferredType::Named(b)) => a_base == b_base

// New: dispatch to SubtypeContext
(InferredType::Named(a), InferredType::Named(b)) =>
    ctx.is_subtype(a, b)  // MRO lookup + protocol check + generic variance
```

### 4h. Verification

- E0014: `x: Animal = Dog()` should pass
- E0014: `f: Standard2 = pos_only  # E` should still fail (not structurally compatible)
- Conformance files: `protocols_subtyping.py`, `callables_subtyping.py`, `protocols_definition.py`
- Expected: ~50 FPs fixed

---

## Phase 5: Wire Into Rules

### 5a. E0014 (Assignment Type Mismatch)

Replace:
- `infer_rhs()` → `inference_engine.infer_expr(rhs)`
- `from_annotation()` → `inference_engine.resolve_type(annotation)`
- `is_assignable_to()` → `subtyping.is_subtype_of()`

### 5b. E0013 (Return Type Mismatch)

- Infer return expression types including function calls
- Use narrowing context for narrowed return values
- Stop skipping call expressions

### 5c. E0053 (assert_type)

- Re-enable the rule (currently disabled with comment)
- Use `inference_engine.infer_expr()` at the call site
- Compare inferred type to expected type argument

### 5d. Verification

- Run full conformance suite
- Expected: ~10 additional FPs fixed from edge cases
- Total Phase 2 target: ~125 FPs eliminated

---

## Execution Order

| Phase | Unblocks | Effort | Est. FPs Fixed | Dependencies |
|-------|----------|--------|----------------|--------------|
| 1. NarrowingEngine | E0053, E0013 narrowing | High | ~15 | None |
| 2. Expression inference | E0014 calls, E0013 calls | High | ~40 | None |
| 3. ConstraintSolver | Generic function calls | High | ~10 | Phase 2 |
| 4. Class hierarchy + structural subtyping | Named-to-Named, protocols, TypedDict, callables | High | ~50 | Phase 2 |
| 5. Wire into rules | All remaining | Medium | ~10 | Phases 1-4 |

Phases 1 and 2 are independent and can be parallelized. Phase 3 depends on Phase 2 (needs expression inference to collect constraints). Phase 4 depends on Phase 2 (needs resolved types). Phase 5 depends on all prior phases.

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Expression inference introduces false negatives (misses real errors) | Gate behind feature flag initially; run conformance suite with flag on/off; missed count must not increase |
| Narrowing state explosion in deeply nested control flow | Cap branch stack depth; fall back to unnarrowed type beyond limit |
| MRO resolution is expensive for deep hierarchies | Cache per class in `ResolvedModule`; invalidate on file change via Salsa |
| ConstraintSolver doesn't converge for complex generic chains | Bound recursion depth; emit diagnostic on unsolvable TypeVars rather than crash |
| Cross-module type resolution adds latency | Keep same-module fast path; cross-module resolution is already cached by import graph |

---

## TODO

- [ ] **Phase 1: NarrowingEngine** (~15 FPs)
  - [ ] 1a. `NarrowingContext` data structure with push/pop/join
  - [ ] 1b. isinstance narrowing (§7.1)
  - [ ] 1c. None narrowing (§7.2)
  - [ ] 1d. Truthiness narrowing (§7.3)
  - [ ] 1e. Assignment narrowing (§7.4)
  - [ ] 1f. Assert narrowing (§7.8)
  - [ ] 1g. TypeGuard narrowing — positive branch only (§7.6)
  - [ ] 1h. TypeIs narrowing — bidirectional (§7.7)
  - [ ] 1i. Pattern match narrowing + exhaustiveness (§7.5)
  - [ ] 1j. Scope limitations — no narrowing across function boundaries or loops (§7.10)
- [ ] **Phase 2: Expression Type Inference** (~40 FPs)
  - [ ] 2a. Function call return type resolution (same-module)
  - [ ] 2b. Constructor call resolution (`ClassName()` → class type)
  - [ ] 2c. Cross-module function call return types
  - [ ] 2d. Method call resolution (`obj.method()`)
  - [ ] 2e. Attribute access type resolution
  - [ ] 2f. Subscript type resolution (list/dict/TypedDict/tuple)
  - [ ] 2g. Binary/unary operation return types (builtin table)
  - [ ] 2h. Conditional expression (`a if cond else b` → union)
  - [ ] 2i. Walrus operator type propagation
- [ ] **Phase 3: ConstraintSolver** (~10 FPs)
  - [ ] 3a. Constraint collection from call arguments
  - [ ] 3b. Lower-bound join / upper-bound meet solving
  - [ ] 3c. Bidirectional constraint from expected return type
  - [ ] 3d. Constrained TypeVar matching (§6.2)
  - [ ] 3e. Bound TypeVar upper-bound check (§6.3)
  - [ ] 3f. TypeVar defaults (PEP 696, §6.5)
- [ ] **Phase 4: Class Hierarchy and Structural Subtyping** (~50 FPs)
  - [ ] 4a. `SubtypeContext` data structure with MRO cache, protocol member tables
  - [ ] 4b. Nominal subtyping via C3 MRO resolution (§9.1) + builtin MRO hardcoding
  - [ ] 4c. Protocol structural subtyping: member collection, method/attribute/property matching (§9.2)
  - [ ] 4d. Generic subtyping with variance-aware TypeVar position checking (§9.4)
  - [ ] 4e. TypedDict structural subtyping: Required/NotRequired/ReadOnly/extra_items (§9.3)
  - [ ] 4f. Callable subtyping: *args/**kwargs, defaults, Protocol.__call__ (§9.6)
  - [ ] 4g. Wire `is_subtype_of()` to replace Named-to-Named string comparison
  - [ ] 4h. Conformance verification: protocols_subtyping, callables_subtyping
- [ ] **Phase 5: Wire Into Rules** (~10 FPs)
  - [ ] 5a. E0014 — use inference engine + subtyping for assignment checks
  - [ ] 5b. E0013 — infer return expression types including calls
  - [ ] 5c. E0053 — re-enable `assert_type()` with inference engine
  - [ ] 5d. Full conformance suite verification — FP count target: < 71
