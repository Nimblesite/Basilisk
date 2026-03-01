# Road to 75% PEP Conformance

> **Current score**: 51.7% (75/145 files pass)
> **Target score**: 75.2% (109/145 files pass)
> **Files needed**: 34 more passing files
> **Previous milestone**: 50% achieved via structural/syntactic/resolver-level checks alone

---

## Where We Are

We blew past 50% with pure structural analysis — no type inference engine at all. The wins came from:
- Dataclass decorator argument validation (frozen, kw_only, match_args, order)
- ReadOnly TypedDict mutation detection
- Final/ClassVar annotation form validation
- Overload structure checking
- Enum member validation

**What's left is harder.** The remaining 70 failing files increasingly need capabilities beyond pattern-matching on AST structure. To reach 75%, we must build **three new capability layers** on top of the existing structural foundation.

---

## Current Category Breakdown

| Category | Pass/Total | % | Notes |
|---|---|---|---|
| (core) | 7/7 | 100% | Done |
| enums | 6/6 | 100% | Done |
| exceptions | 1/1 | 100% | Done |
| historical | 1/1 | 100% | Done |
| overloads | 4/4 | 100% | Done |
| qualifiers | 3/3 | 100% | Done |
| directives | 9/10 | 90% | 1 file needs @deprecated |
| specialtypes | 4/5 | 80% | 1 file needs type[] checks + FP fixes |
| annotations | 3/5 | 60% | 2 files need forward-ref + generator checks |
| dataclasses | 9/16 | 56% | 7 files need transform + postinit + slots |
| typeddicts | 8/14 | 57% | 6 files need operations + subtyping |
| classes | 1/2 | 50% | 1 file needs ClassVar deep checks |
| constructors | 3/6 | 50% | 3 files need TypeVar + constructor dispatch |
| generics | 10/30 | 33% | 20 files — the biggest category by far |
| tuples | 1/3 | 33% | 2 files need tuple subtyping |
| protocols | 3/11 | 27% | 8 files need structural subtyping |
| literals | 1/4 | 25% | 3 files need literal semantics |
| namedtuples | 1/4 | 25% | 3 files need field synthesis |
| aliases | 0/7 | 0% | All need type alias resolution |
| callables | 0/4 | 0% | All need callable subtyping |
| narrowing | 0/2 | 0% | Both need TypeGuard/TypeIs checking |

---

## Three Capability Layers Needed

### Layer 1: Structural Completion (no new infrastructure)

Finish the remaining structural/syntactic checks that need only resolver data we already have or can trivially add. These are the same kind of work that got us to 50%.

**Examples**: ClassVar nesting rules, PEP 695 bound violations, InitVar tracking, NamedTuple immutability, tuple annotation syntax, literal value comparison.

### Layer 2: Lightweight Type Comparison

Not full TypeVar constraint solving. Just: "given two known annotation types, are they compatible?" This means:
- `int` is not assignable to `str`
- `float` is not assignable to `int` (Python numeric tower aside)
- `Literal[3]` is not assignable to `Literal[4]`
- `Literal[0]` is not assignable to `Literal[False]` (int vs bool)
- TypedDict `A` is not assignable to TypedDict `B` if fields differ
- `dict[str, int]` is not compatible with a TypedDict
- `.clear()` and `del` are illegal on TypedDicts

We already do some of this in E0012/E0014. Extending it to cover annotation-to-annotation comparison unlocks a large number of files.

### Layer 3: Basic Generics Foundation

A minimal TypeVar solver that handles:
- Simple single-TypeVar functions: `def f(x: T) -> T` called with `f(3)` → `T = int`
- Explicit specialization: `Class[int]` binds the TypeVar
- Bound checking: `T: bound=int` rejects `T = str`
- Default TypeVar values (PEP 696)
- PEP 695 new-style type parameter syntax scoping

This is NOT full inference. No bidirectional propagation, no ParamSpec solving, no variance inference, no flow-sensitive narrowing. Just enough to pass the easier generics files.

---

## All 70 Failing Files — Deep Analysis

### Tier 1: Structural Completion (10 files, high confidence)

These files need only structural/syntactic checks or trivial resolver extensions.

| File | Missed | FP | What's Needed | Key Gaps |
|---|---|---|---|---|
| `classes_classvar.py` | 12 | 0 | ClassVar arity, nesting, position, TypeVar-in-ClassVar | Line 140 needs protocol check — risk |
| `literals_semantics.py` | 4 | 0 | Literal value assignment checks, `Literal[0]` vs `Literal[False]` | All achievable with value comparison |
| `tuples_type_form.py` | 11 | 0 | Tuple annotation syntax (lines 41-46) + literal tuple element checks | Pure syntactic + lightweight comparison |
| `namedtuples_usage.py` | 8 | 0 | NamedTuple immutability, index bounds, unpack count | All structural once field count known |
| `dataclasses_usage.py` | 4 | 0 | Too-many-args for dataclass ctors, init=False edge cases | 3 Medium, 1 Hard (default_factory type) |
| `dataclasses_postinit.py` | 4 | 0 | InitVar tracking, `__post_init__` parameter validation | Need `is_init_var` on AttributeInfo |
| `dataclasses_slots.py` | 4 | 0 | `slots=True` tracking, `__slots__` cross-reference | Need method-body self-assignment analysis |
| `generics_syntax_declarations.py` | 8 | 0 | PEP 695 bound violations — **infrastructure already exists in resolver!** | Just needs visitor population + checker rule |
| `namedtuples_define_class.py` | 14 | 0 | NamedTuple field synthesis + arity/type/bounds/name validation | Foundational NamedTuple support |
| `directives_deprecated.py` | 12 | 0 | `@deprecated` use-site tracking across module boundaries | Needs cross-module decorator resolution |

**Expected gain: 8-10 files** (some have 1-2 errors that may need lightweight comparison)

### Tier 2: Lightweight Type Comparison (12 files, medium confidence)

These files need the ability to compare known annotation types without TypeVar solving.

| File | Missed | FP | What's Needed | Achievability |
|---|---|---|---|---|
| `typeddicts_usage.py` | 5 | 0 | isinstance(x, TD) prohibition, TypeVar(bound=TD), key type checks | 2 structural, 3 need type tracking |
| `typeddicts_operations.py` | 11 | 0 | .clear() prohibition, del on required key, key/value type checks | 2-3 structural, rest need type comparison |
| `typeddicts_type_consistency.py` | 9 | 0 | TypedDict→dict prohibition, structural subtyping | Lines 76-78 achievable, rest need subtyping |
| `typeddicts_readonly_inheritance.py` | 10 | 0 | ReadOnly re-declaration rules, inheritance conflict detection | Lines 94/98/106/119/132 achievable |
| `typeddicts_readonly_consistency.py` | 7 | 0 | Full ReadOnly structural subtyping (PEP 705) | Hardest TypedDict file — all need subtyping |
| `typeddicts_extra_items.py` | 23 | 0 | PEP 728 closed/extra_items — large feature | Massive scope, defer unless easy subset works |
| `annotations_forward_refs.py` | 6 | 4 | String annotation content validation, fix FPs | Achievable once FPs fixed |
| `specialtypes_type.py` | 8 | 5 | type[] arity, attribute checks on type[object] | Fix FPs first, then 2-3 structural checks |
| `literals_literalstring.py` | 7 | 3 | LiteralString inside Literal[] (structural), basic type checks | Lines 36/37/73/74 structural |
| `literals_interactions.py` | 4 | 4 | Tuple literal index bounds with Literal[N] | Achievable but FPs must be fixed first |
| `namedtuples_define_functional.py` | 9 | 0 | Functional namedtuple field list parsing + arity checks | Achievable for arity; types need comparison |
| `callables_annotation.py` | 16 | 0 | Callable form validation (5 structural), basic call checking | Lines 55-59 syntactic, rest needs Callable infra |

**Expected gain: 6-8 files** (files with FPs are risky — must fix FPs AND catch all errors)

### Tier 3: dataclass_transform + Narrowing Foundations (8 files, medium confidence)

| File | Missed | FP | What's Needed | Achievability |
|---|---|---|---|---|
| `dataclasses_transform_class.py` | 6 | 0 | Extend `apply_dataclass_transform` to base classes | All structural once class-based transform works |
| `dataclasses_transform_meta.py` | 6 | 0 | Extend `apply_dataclass_transform` to metaclasses | Same pattern as class-based |
| `dataclasses_transform_func.py` | 4 | 1 | Parse `frozen_default`, fix FP | 2 Medium, 2 Hard |
| `dataclasses_transform_converter.py` | 9 | 0 | Full converter type analysis | All Hard — skip for 75% |
| `narrowing_typeguard.py` | 4 | 2 | TypeGuard return → must have narrowable param (structural) | Lines 102/107 structural; 128/148 need subtyping |
| `narrowing_typeis.py` | 9 | 2 | TypeIs structural constraints + invariance rules | Lines 105/110 structural; rest needs subtyping |
| `tuples_type_compat.py` | 16 | 0 | Variadic tuple subtype algorithm | Core rules achievable, variadic adds complexity |
| `annotations_generators.py` | 10 | 0 | Generator return type checking | Lines 86/91 achievable; rest needs yield-type analysis |

**Expected gain: 3-5 files**

### Tier 4: Basic Generics Foundation (20 files, variable confidence)

Ranked by proximity to passing:

| File | Missed | FP | Key Gap | Achievability |
|---|---|---|---|---|
| `generics_defaults_specialization.py` | 2 | 2 | Fix 2 FPs + catch 2 TypeVar default errors | Closest! Fix FPs may flip it |
| `generics_defaults.py` | 4 | 0 | TypeVar bound/default compatibility | Needs `default_text` on TypeVarCallInfo |
| `generics_defaults_referential.py` | 7 | 0 | Default referencing other TypeVars | Moderate |
| `generics_base_class.py` | 6 | 0 | Base class TypeVar checking | 2 structural, 4 inference |
| `generics_basic.py` | 8 | 0 | TypeVar not in Generic list (2 structural), rest inference | Hard |
| `generics_scoping.py` | 10 | 0 | TypeVar scoping rules | Hard |
| `generics_syntax_scoping.py` | 7 | 0 | PEP 695 scoping | Hard |
| `generics_type_erasure.py` | 7 | 0 | Type erasure rules | Hard |
| `generics_self_usage.py` | 10 | 1 | Self type binding | Hard |
| `generics_variance.py` | 8 | 0 | Variance checking | Hard |
| `generics_variance_inference.py` | 23 | 0 | Full variance inference | Very Hard |
| `generics_syntax_infer_variance.py` | 17 | 0 | PEP 695 variance inference | Very Hard |
| `generics_paramspec_basic.py` | 7 | 0 | ParamSpec basics | Hard |
| `generics_paramspec_components.py` | 16 | 0 | ParamSpec decomposition | Very Hard |
| `generics_paramspec_semantics.py` | 9 | 0 | ParamSpec semantics | Very Hard |
| `generics_paramspec_specialization.py` | 5 | 0 | ParamSpec specialization | Hard |
| `generics_typevartuple_args.py` | 10 | 0 | TypeVarTuple args | Very Hard |
| `generics_typevartuple_basic.py` | 14 | 0 | TypeVarTuple basics | Very Hard |
| `generics_typevartuple_specialization.py` | 6 | 12 | TypeVarTuple + lots of FPs | Very Hard |

**Expected gain: 3-5 files** (defaults files + syntax_declarations from Tier 1 + a couple structural wins)

### Tier 5: Skip for 75% (needs deep inference engine)

| Category | Files | Why |
|---|---|---|
| `protocols_*` (8 failing) | 8 | Full structural subtyping algorithm needed |
| `callables_kwargs/protocol/subtyping` | 3 | Callable subtyping + Unpack[TypedDict] |
| `constructors_*` (3 failing) | 3 | TypeVar + constructor dispatch |
| `aliases_*` (7 failing) | 7 | Type alias resolution, recursive aliases |

**Exception within Tier 5** — these specific files have a few structural wins that could flip them IF combined with other work:
- `protocols_runtime_checkable.py` (6 missed) — add `is_protocol` + `is_runtime_checkable` to ClassInfo, catches lines 23/55/61 (3 of 6). Still needs 3 more.
- `protocols_merging.py` (6 missed) — non-protocol class in Protocol bases (line 67). Still needs 5 more.

---

## Concrete Work Items

### Phase 1: Structural Completion (target: +10 files → 85/145 = 58.6%)

#### WI-1: PEP 695 Bound Violations Rule
**Target**: `generics_syntax_declarations.py` (8 missed)
**Status**: Infrastructure already exists! `pep695_bound_violations` vector is defined in `ResolvedModule`, violation types are defined (`ListLiteralBound`, `EmptyTuple`, `SingleElementTuple`, `NonLiteralConstraint`, `InvalidConstraintElement`, `OuterScopeTypeVarInBound`). Just needs:
1. Populate `pep695_bound_violations` in the visitor (walk `class.type_params`)
2. Write a checker rule consuming the violations
**Effort**: Small — types and plumbing exist, just needs AST walking + rule
**Expected gain**: 1 file (catches 6-7 of 8 missed)

#### WI-2: NamedTuple Field Synthesis + Immutability
**Target**: `namedtuples_usage.py` (8 missed), `namedtuples_define_class.py` (14 missed)
**New capabilities**:
- Synthesize field list from class-based NamedTuple definition
- Track field count, names, types
- Enforce immutability: reject `p.x = 3`, `p[0] = 3`, `del p.x`, `del p[0]`
- Index bounds: `p[N]` where N >= len(fields) or N < -len(fields)
- Unpack count: `x, y = p` where target count != field count
- Field name validation: underscore prefix, non-default after default
- Constructor arity checking
**Resolver work**: Add `is_named_tuple: bool` to ClassInfo, extract fields with types
**Effort**: Medium — foundational NamedTuple support
**Expected gain**: 2 files

#### WI-3: Tuple Annotation Syntax Validation
**Target**: `tuples_type_form.py` (11 missed)
**New capabilities**:
- `tuple[int, int, ...]` — ellipsis requires exactly one type before it
- `tuple[...]` — bare ellipsis invalid
- `tuple[..., int]` — ellipsis must be last
- `tuple[int, ..., int]` — ellipsis in middle
- `tuple[*tuple[str], ...]` — star-unpack with ellipsis
- Literal tuple length/type checking against annotated tuple type
**Effort**: Small-Medium — syntactic checks + literal comparison
**Expected gain**: 1 file

#### WI-4: Literal Value Semantics
**Target**: `literals_semantics.py` (4 missed)
**New capabilities**:
- `Literal[3] = 4` — wrong literal value
- `Literal[False] = a` where `a: Literal[0]` — int/bool distinction
- `a += 3` where `a: Literal[3,4,5]` — augmented assignment widens literal type
**Effort**: Small — well-specified value comparison rules
**Expected gain**: 1 file

#### WI-5: ClassVar Deep Validation
**Target**: `classes_classvar.py` (12 missed)
**New capabilities**:
- `ClassVar[int, str]` — too many arguments
- `ClassVar[T]` / `ClassVar[list[T]]` / `ClassVar[Callable[P, Any]]` — TypeVar/ParamSpec inside ClassVar
- `Final[ClassVar[...]]` / `list[ClassVar[int]]` — nesting violations
- ClassVar in function parameter, local variable, return type, self attribute
- ClassVar in TypeAlias
- Instance access to ClassVar attribute
**Risk**: Line 140 (Protocol implementation) may need protocol checking. Line 52 needs type comparison.
**Effort**: Medium
**Expected gain**: 1 file (must catch all 12 — ambitious)

#### WI-6: Dataclass PostInit + Slots
**Target**: `dataclasses_postinit.py` (4 missed), `dataclasses_slots.py` (4 missed)
**New capabilities**:
- `is_init_var: bool` on AttributeInfo for `InitVar[T]` fields
- `__post_init__` parameter count must match InitVar field count
- Attribute access on InitVar field is error (not a real attribute)
- `slots=True` tracking on ClassInfo
- `slots=True` + explicit `__slots__` already defined = error
- Attribute assignment to undeclared slot in `__init__`
- `__slots__` access when no slots
**Effort**: Medium — two related but distinct resolver extensions
**Expected gain**: 2 files

#### WI-7: Dataclass Usage Completion
**Target**: `dataclasses_usage.py` (4 missed)
**New capabilities**:
- Too-many-positional-args for dataclass constructors
- `init=False` dataclass called with arguments
- Track `init=False` per field for accurate arg counting
**Effort**: Small-Medium
**Expected gain**: 1 file

#### WI-8: @deprecated Use-Site Detection
**Target**: `directives_deprecated.py` (12 missed)
**New capabilities**:
- Recognize `@deprecated` / `@typing_extensions.deprecated` decorator
- Track across module boundaries (cross-module stubs in `_directives_deprecated_library`)
- Flag: function calls, class instantiation, method calls, dunder dispatch (`__add__`, `__call__`), property access
**Risk**: Cross-module resolution is the hard part. Dunder dispatch (lines 41/42/44/47/48) needs per-dunder tracking.
**Effort**: Large — cross-module decorator resolution + use-site tracking
**Expected gain**: 1 file

### Phase 2: Lightweight Type Comparison (target: +8 files → 93/145 = 64.1%)

#### WI-9: Annotation Type Comparison Engine
**Foundation for multiple files.** Build a function `types_compatible(lhs: &str, rhs: &str) -> bool` that handles:
- Exact match: `int` == `int`
- Builtin hierarchy: `bool` < `int` (but NOT `int` < `bool`)
- `None` / `NoneType` equivalence
- `Literal[X]` value comparison
- Union containment: `int` is in `int | str`
- Container invariance: `list[int]` != `list[str]`
- `Optional[X]` = `X | None`

This is NOT TypeVar solving. It's a lookup table + simple structural comparison on annotation text.

#### WI-10: TypedDict Operations + Usage
**Target**: `typeddicts_operations.py` (11 missed), `typeddicts_usage.py` (5 missed)
**New capabilities using WI-9**:
- `isinstance(x, TypedDict)` prohibition → structural
- `TypeVar(bound=TypedDict)` prohibition → structural
- `.clear()` / `.pop()` on TypedDict → method call tracking
- `del td["required_key"]` → required key + del detection
- Unknown key in subscript access/assignment
- Wrong value type for known key (uses WI-9 type comparison)
- Missing required key in dict literal
- Extra key in dict literal
- Variable (non-literal) key in TypedDict construction
**Effort**: Large — multiple distinct checks, each medium
**Expected gain**: 2 files (need ALL errors in each)

#### WI-11: TypedDict Subtyping Foundation
**Target**: `typeddicts_type_consistency.py` (9 missed), `typeddicts_readonly_inheritance.py` (10 missed)
**New capabilities using WI-9**:
- TypedDict not assignable to `dict[str, X]` (categorical rule)
- ReadOnly field re-declaration rules (mutable→ReadOnly illegal, Required→NotRequired illegal)
- Multiple inheritance field conflicts with ReadOnly/Required qualifiers
- Dict literal extra-key detection for TypedDict-annotated assignments
**Effort**: Large
**Expected gain**: 2 files (type_consistency has 9 errors, many need subtyping; readonly_inheritance has structural subset)

#### WI-12: Forward Reference + String Annotation Validation
**Target**: `annotations_forward_refs.py` (6 missed, 4 FP)
**New capabilities**:
- Parse string annotation contents, check for invalid forms (list/tuple display, lambda, integers, f-strings, `or` keyword, module names)
- Detect `"ClassA" | int` bitwise OR with string literal at runtime
- Fix 4 false positives (likely overeager strictness rules on valid annotations)
**Effort**: Medium
**Expected gain**: 1 file (must fix FPs too)

#### WI-13: LiteralString Validation
**Target**: `literals_literalstring.py` (7 missed, 3 FP)
**New capabilities**:
- `Literal["hello", LiteralString]` — LiteralString inside Literal is invalid (structural)
- `Literal[LiteralString]` — same
- `int` / `bytes` not assignable to LiteralString (uses WI-9)
- Fix 3 false positives
**Effort**: Medium
**Expected gain**: 1 file

#### WI-14: False Positive Cleanup + Literal Index Bounds
**Target**: `literals_interactions.py` (4 missed, 4 FP), `specialtypes_type.py` (8 missed, 5 FP)
**New capabilities**:
- Tuple literal index-out-of-bounds with `Literal[N]` or integer constant
- `type[int, str]` arity check (too many args to type[])
- Fix false positives in assert_type handling of `type[Any]` and Literal narrowing
**Risk**: FP fixes are unpredictable — may require deep debugging
**Effort**: Medium
**Expected gain**: 2 files (if FPs are fixable)

### Phase 3: Generics Foundation (target: +8 files → 101/145 = 69.7%)

#### WI-15: dataclass_transform on Classes + Metaclasses
**Target**: `dataclasses_transform_class.py` (6), `dataclasses_transform_meta.py` (6), `dataclasses_transform_func.py` (4, 1 FP)
**Current state**: `apply_dataclass_transform` in the resolver only handles `@dataclass_transform` on **functions**. It does NOT handle class-based or metaclass-based transforms.
**New capabilities**:
- Detect `@dataclass_transform` on class definitions (ModelBase pattern)
- Detect `@dataclass_transform` on metaclasses
- Parse `frozen_default`, `kw_only_default` from `@dataclass_transform()` arguments
- Propagate frozen/kw_only/order semantics to subclasses
- Fix 1 FP in func variant
**Effort**: Large — extends existing transform infrastructure to 2 new patterns
**Expected gain**: 3 files

#### WI-16: Basic TypeVar Default Checking
**Target**: `generics_defaults.py` (4 missed), `generics_defaults_specialization.py` (2 missed, 2 FP)
**New capabilities**:
- Add `default_text` to `TypeVarCallInfo`
- TypeVar bound/default incompatibility check: `T(bound=int, default=str)` is error
- TypeVar constraint/default incompatibility: `T(int, str, default=bytes)` is error
- Fix 2 FPs in defaults_specialization
**Effort**: Medium
**Expected gain**: 2 files (defaults_specialization is the closest — only 2 missed + 2 FP!)

#### WI-17: TypeVar-in-Generic-List Check
**Target**: helps `generics_basic.py` (8 missed) and `generics_base_class.py` (6 missed)
**New capabilities**:
- `class Bad(Iterable[T_co], Generic[S_co])` — T_co appears in base subscript but not in Generic[] list
- Cross-reference `base_expression_names` vs `generic_params`
**Note**: This catches 2-3 errors in each file but won't be enough alone to pass them. Still valuable infrastructure.
**Effort**: Small
**Expected gain**: 0 files directly (reduces miss count, enabling future pass)

#### WI-18: Narrowing Type Structural Checks
**Target**: `narrowing_typeguard.py` (4 missed, 2 FP), `narrowing_typeis.py` (9 missed, 2 FP)
**New capabilities**:
- TypeGuard/TypeIs return type → function must have a narrowable (non-self/cls) parameter
- TypeIs invariance: `TypeIs[bool]` not assignable to `TypeIs[int]`
- TypeIs narrowed type must be consistent with input parameter type
- Fix FPs
**Risk**: Only structural checks (lines 102/107/105/110) are easy. Full TypeGuard subtyping and TypeIs consistency need type comparison. Need ALL errors per file.
**Effort**: Medium
**Expected gain**: 0-2 files (depends on whether all errors are achievable)

#### WI-19: Callable Annotation Form Validation
**Target**: `callables_annotation.py` (16 missed)
**New capabilities**:
- Invalid `Callable` annotation forms (lines 55-59): `Callable[int]`, `Callable[int, str, bool]`, etc.
- Basic Callable call-site checking: arity of `Callable[[int, str], R]` variable
**Risk**: Lines 91/93/159/172/187/189 need Concatenate/ParamSpec/Protocol. Need ALL 16. Very unlikely to pass.
**Effort**: Medium for form validation, Large for call checking
**Expected gain**: 0 files (partial wins reduce miss count for future)

### Phase 4: Stretch Goals (target: +8 files → 109/145 = 75.2%)

These require either the generics foundation to mature or targeted structural wins in unexpected places.

#### WI-20: Functional NamedTuple Parsing
**Target**: `namedtuples_define_functional.py` (9 missed)
**New capabilities**:
- Parse `namedtuple("Point", ["x", "y"])` field list from call arguments
- Parse `NamedTuple("Point", [("x", int), ("y", int)])` with types
- Arity + type checking against synthesized constructor
**Effort**: Medium
**Expected gain**: 1 file

#### WI-21: Tuple Type Compatibility
**Target**: `tuples_type_compat.py` (16 missed)
**New capabilities**:
- Fixed-length tuple subtyping: `tuple[int]` vs `tuple[int, int]`
- Homogeneous tuple: `tuple[int, ...]` compatibility rules
- Variadic tuple with star-unpack: `tuple[int, *tuple[int, ...]]`
**Effort**: Large — full variadic tuple subtype algorithm
**Expected gain**: 1 file

#### WI-22: TypedDict Extra Items (PEP 728)
**Target**: `typeddicts_extra_items.py` (23 missed)
**New capabilities**:
- Parse `extra_items=T` keyword from TypedDict syntax
- `closed=True/False` semantics and inheritance chain validation
- `extra_items=Required[int]` / `NotRequired[int]` prohibition
- Dict literal construction against extra_items types
**Risk**: 23 errors is a massive scope. PEP 728 is still experimental.
**Effort**: Very Large
**Expected gain**: 1 file (high effort, low confidence)

#### WI-23: TypedDict ReadOnly Consistency (PEP 705)
**Target**: `typeddicts_readonly_consistency.py` (7 missed)
**New capabilities**:
- Full ReadOnly-aware structural subtyping per PEP 705
- 4-condition compatibility matrix: requiredness + mutability cross-product
- Field-by-field comparison with invariance for mutable, covariance for ReadOnly
**Effort**: Large
**Expected gain**: 1 file

#### WI-24: Protocol is_protocol + runtime_checkable Infrastructure
**Target**: helps `protocols_runtime_checkable.py` (6 missed), `protocols_merging.py` (6 missed)
**New capabilities**:
- Add `is_protocol: bool` and `is_runtime_checkable: bool` to ClassInfo
- `isinstance(x, Proto)` where Proto is non-@runtime_checkable → error
- `issubclass(x, DataProto)` where DataProto has data attributes → error
- Non-protocol class in Protocol bases
**Note**: Catches 3-4 errors per file but likely not enough alone.
**Effort**: Small (resolver change) + Medium (rules)
**Expected gain**: 0-1 files

#### WI-25: Generator Return Type Check
**Target**: helps `annotations_generators.py` (10 missed)
**Quick wins**: Lines 86/91 — detect `yield` in a function declared to return a non-generator/non-iterator type
**Note**: Only catches 2 of 10. Won't pass file.
**Effort**: Small
**Expected gain**: 0 files (reduces miss count)

#### WI-26: Generics Scoping + Syntax
**Target**: `generics_scoping.py` (10), `generics_syntax_scoping.py` (7)
**New capabilities**:
- TypeVar scoping rules across nested classes/functions
- PEP 695 type parameter scope resolution
**Effort**: Large
**Expected gain**: 0-2 files

---

## Score Projection

| Phase | Work Items | Files Gained | Running Total | Score |
|---|---|---|---|---|
| Baseline | — | 0 | 75 | 51.7% |
| **Phase 1: Structural** | WI-1 through WI-8 | +10 | 85 | 58.6% |
| **Phase 2: Type Comparison** | WI-9 through WI-14 | +8 | 93 | 64.1% |
| **Phase 3: Generics Foundation** | WI-15 through WI-19 | +8 | 101 | 69.7% |
| **Phase 4: Stretch Goals** | WI-20 through WI-26 | +8 | 109 | **75.2%** |

### Conservative Estimate

Not every file will pass — some will have 1-2 errors we can't catch without deeper inference. Realistic:

| Phase | Optimistic | Conservative | Most Likely |
|---|---|---|---|
| Phase 1 | +10 | +7 | +8 |
| Phase 2 | +8 | +4 | +6 |
| Phase 3 | +8 | +4 | +5 |
| Phase 4 | +8 | +3 | +5 |
| **Total** | **+34** | **+18** | **+24** |
| **Score** | **75.2%** | **64.1%** | **68.3%** |

To reliably hit 75%, we need the optimistic case in Phases 1-2 (achievable with careful work) and solid execution in Phase 3. Phase 4 provides the safety margin.

---

## Execution Order (by bang-for-buck)

### Sprint 1 — Quick Structural Wins (+5 files)
1. **WI-1**: PEP 695 bound violations (infrastructure exists, just wire it up)
2. **WI-4**: Literal value semantics (4 small checks)
3. **WI-3**: Tuple annotation syntax (6 syntactic checks)
4. **WI-7**: Dataclass usage completion (arity checks)
5. **WI-2**: NamedTuple field synthesis + immutability (foundational)

### Sprint 2 — Dataclass + Resolver Extensions (+5 files)
6. **WI-6**: Dataclass postinit + slots (InitVar + slots tracking)
7. **WI-5**: ClassVar deep validation (extend e0036)
8. **WI-8**: @deprecated use-site detection (cross-module)
9. **WI-9**: Annotation type comparison engine (foundation for Phase 2)

### Sprint 3 — Type Comparison Payoff (+5 files)
10. **WI-10**: TypedDict operations + usage
11. **WI-12**: Forward reference validation
12. **WI-13**: LiteralString validation
13. **WI-14**: FP cleanup + literal index bounds

### Sprint 4 — Generics + Transform (+5 files)
14. **WI-15**: dataclass_transform class + metaclass
15. **WI-16**: TypeVar default checking
16. **WI-11**: TypedDict subtyping foundation

### Sprint 5 — Push to 75% (+5 files)
17. **WI-18**: Narrowing structural checks
18. **WI-20**: Functional NamedTuple
19. **WI-21**: Tuple type compatibility
20. **WI-22**: TypedDict extra_items OR **WI-23**: ReadOnly consistency

### Sprint 6 — Safety Margin (+4 files)
21. **WI-24**: Protocol infrastructure
22. **WI-26**: Generics scoping
23. **WI-17**: TypeVar-in-Generic check
24. **WI-25**: Generator return type

---

## What 75% Does NOT Cover

Even at 75%, these categories will remain mostly unresolved:

| Category | Expected at 75% | Why |
|---|---|---|
| aliases (0/7) | 0/7 | Full type alias resolution, recursive aliases |
| callables (0/4) | 0-1/4 | Callable subtyping, Unpack[TypedDict], Protocol __call__ |
| protocols (3/11) | 3-4/11 | Full structural subtyping algorithm |
| constructors (3/6) | 3/6 | TypeVar + constructor dispatch |
| generics (10/30) | 14-16/30 | Only defaults + syntax + basics covered |
| narrowing (0/2) | 0-2/2 | TypeGuard/TypeIs need type comparison |

These are **Phase 2+ work** — the full type inference engine. They represent the path from 75% to 90%+.

---

## Key Risk Factors

### 1. The "All-or-Nothing" Problem
A file passes only when **every** `# E` line has a diagnostic. Missing even one line fails the entire file. For files with 10+ required errors, the chance of missing one goes up. Mitigation: prioritize files with fewer required errors first.

### 2. False Positive Regression
Adding new rules to catch missed errors risks introducing FPs on currently-passing files. Every new rule must be tested against the full suite. **Files with existing FPs** (literals_interactions, specialtypes_type, annotations_forward_refs, etc.) are double work — fix FPs AND catch new errors. Mitigation: run conformance after every change.

### 3. Cross-Module Resolution
`directives_deprecated.py` and several other files reference symbols defined in stub modules (`_directives_deprecated_library`, etc.). The resolver must follow imports to read decorator metadata from those modules. This is a significant infrastructure investment.

### 4. The Generics Cliff
The generics category has 20 failing files but most need real TypeVar constraint solving. Our "basic generics foundation" (Layer 3) will only catch the easiest cases. The jump from 70% to 75% may require more generics work than estimated.

### 5. PEP 728 (extra_items) Scope
`typeddicts_extra_items.py` has 23 required errors — the largest single file. Implementing the full PEP 728 `closed`/`extra_items` feature is a major undertaking that may not be worth it for one file.

---

## Success Criteria

75% means passing **109 of 145 files**. The minimum viable path requires:
- Phase 1 (structural): 7+ files
- Phase 2 (type comparison): 5+ files
- Phase 3 (generics foundation): 5+ files
- Phase 4 (stretch): 3+ files

If any phase underperforms, the others must compensate. The plan has ~10 files of slack between the optimistic and required counts.

**Run the conformance suite after every work item. Never merge a rule that causes a regression.**
