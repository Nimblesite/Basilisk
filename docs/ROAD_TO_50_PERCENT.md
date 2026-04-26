# Road to 50% PEP Conformance — Without Type Inference

> **Current score**: 28.3% (41/145 files pass)
> **Target score**: 50.3% (73/145 files pass)
> **Files needed**: 32 more passing files
> **Constraint**: No type inference engine. Only structural/syntactic/resolver-level checks.

---

## Strategy

Type inference (TypeVar solving, flow-sensitive narrowing, bidirectional constraint propagation) is a massive undertaking documented in `TYPE_INFERENCE.md`. We do NOT need it to reach 50%.

The conformance suite tests many things that are **purely structural**:
- "You can't put `ClassVar` inside `Final`"
- "You can't reassign a `Final` variable"
- "Frozen dataclasses reject attribute assignment"
- "ReadOnly TypedDict fields reject mutation"
- "`@deprecated` usage must produce a diagnostic"
- "Overload with no matching signature is an error"

These are all pattern-matching on AST structure + resolver data. No constraint solving required.

---

## What "Not Type Inference" Means

| Can do now (resolver-level) | Needs type inference (skip) |
|---|---|
| Decorator argument validation | TypeVar constraint solving |
| Annotation form validation (`Final[str, int]` = bad) | Bidirectional type propagation |
| Reassignment detection (Final, frozen) | Narrowing (isinstance, TypeGuard, TypeIs) |
| Structural inheritance checks | Generic specialization |
| Key existence / mutation on TypedDict | Protocol structural subtyping |
| Overload signature structure | Overload dispatch resolution |
| Enum member form validation | Callable subtyping |
| `@deprecated` usage detection | ParamSpec / TypeVarTuple solving |

---

## Priority Tiers

### Tier 1 — Low-Hanging Fruit (1–2 missed errors per file)

These files are ONE or TWO errors away from passing. Each is a surgical fix.

| File | Missed | What's Needed | Inference? |
|---|---|---|---|
| `dataclasses_match_args.py` | 1 | Detect access to `__match_args__` when `match_args=False` | No |
| `dataclasses_order.py` | 1 | Detect `<` comparison when `order=False` | No |
| `enums_expansion.py` | 1 | `assert_type` mismatch on Flag literal | Partial — needs assert_type to understand Literal[Flag] |
| `overloads_basic.py` | 1 | No matching overload for `b[""]` | Yes — skip |
| `typeddicts_readonly_kwargs.py` | 1 | Reject assignment to ReadOnly key via `**kwargs` | No |
| `typeddicts_readonly_update.py` | 1 | Reject `.update()` on ReadOnly TypedDict | No |
| `specialtypes_promotions.py` | 1 | int/float/complex promotion chain error | Yes — skip |
| `generics_upper_bound.py` | 1 | TypeVar upper bound violation | Yes — skip |
| `generics_typevartuple_callable.py` | 1 | TypeVarTuple in Callable | Yes — skip |
| `generics_typevartuple_unpack.py` | 1 | Unpack with TypeVarTuple | Yes — skip |
| `dataclasses_frozen.py` | 2 | Frozen/non-frozen inheritance conflict (tagged groups) | No |
| `dataclasses_hash.py` | 2 | Unhashable dataclass assigned to `Hashable` | Partial |
| `annotations_typeexpr.py` | 2 | Invalid type expression forms | No |
| `enums_member_values.py` | 2 | Wrong value type in typed enum | Partial |
| `overloads_evaluation.py` | 2 | No matching overload + Literal mismatch | Yes — skip |

**Achievable in Tier 1 without inference: ~8 files**

### Tier 2 — Medium Effort (3–5 missed errors per file)

| File | Missed | What's Needed | Inference? |
|---|---|---|---|
| `dataclasses_kwonly.py` | 3 | Reject positional args for `kw_only=True` fields | No |
| `dataclasses_postinit.py` | 4 | Validate `__post_init__` signature vs InitVar fields; reject InitVar access | No |
| `dataclasses_slots.py` | 4 | Reject undeclared attribute assignment with `slots=True`; detect missing `__slots__` | No |
| `dataclasses_final.py` | 5 | Reject assignment to `Final` fields on dataclass instances | No (extends E0054) |
| `historical_positional.py` | 4 | Positional-only parameter enforcement (`/` syntax) | No |
| `directives_assert_type.py` | 5 | 3 are structural (arg count), 2 need type comparison | Partial — 3/5 structural |
| `typeddicts_usage.py` | 5 | Invalid key, wrong value type, isinstance check, TypeVar bound | Partial — 3/5 structural |
| `specialtypes_never.py` | 3 | Never type behavior | Yes — skip |
| `specialtypes_none.py` | 3 | None type behavior | Yes — skip |
| `literals_semantics.py` | 4 | Literal type semantics | Yes — skip |
| `literals_interactions.py` | 4 | Literal interaction with other types | Yes — skip |

**Achievable in Tier 2 without inference: ~7 files** (some partially, counting files where we can catch ALL required errors)

### Tier 3 — Significant Effort (6–12 missed errors per file)

| File | Missed | What's Needed | Inference? |
|---|---|---|---|
| `typeddicts_readonly.py` | 6 | Reject assignment to ReadOnly fields | No |
| `typeddicts_readonly_consistency.py` | 7 | ReadOnly vs mutable TypedDict assignment compat | Partial |
| `typeddicts_readonly_inheritance.py` | 10 | ReadOnly inheritance rules, Required+ReadOnly conflicts | Partial — ~6/10 structural |
| `typeddicts_operations.py` | 11 | Wrong type for key, unknown key, missing required key, clear/del | Partial — ~7/11 structural |
| `enums_members.py` | 6 | Annotated enum member, Literal[EnumMember] checks | Partial |
| `qualifiers_annotated.py` | 8+1fp | Invalid Annotated first arg forms, arg count, instantiation | No — mostly form validation |
| `qualifiers_final_annotation.py` | 20 | Final reassignment, Final in wrong position, Final+ClassVar conflict | No — all structural |
| `classes_classvar.py` | 12 | ClassVar arg count, ClassVar with TypeVar, ClassVar nesting, ClassVar in wrong position | Partial — ~8/12 structural |
| `directives_deprecated.py` | 12 | Detect usage of @deprecated functions/methods/classes | No — resolver + call tracking |
| `dataclasses_transform_class.py` | 6 | `@dataclass_transform` on class | Partial |
| `dataclasses_transform_func.py` | 5 | `@dataclass_transform` on function | Partial |
| `dataclasses_transform_field.py` | 2 | `@dataclass_transform` field specifiers | Partial |
| `dataclasses_transform_meta.py` | 6 | `@dataclass_transform` on metaclass | Partial |
| `dataclasses_transform_converter.py` | 9 | `@dataclass_transform` converter param | Yes — skip |
| `dataclasses_usage.py` | 8 | General dataclass usage validation | Partial |

**Achievable in Tier 3: ~6 files** (the ones that are fully structural)

### Tier 4 — Skip (Requires Type Inference)

These categories are almost entirely inference-dependent. Do not attempt for 50%.

| Category | Files | Why |
|---|---|---|
| `generics_*` (26 failing) | 26 | TypeVar solving, variance inference, ParamSpec, TypeVarTuple |
| `callables_*` (4 failing) | 4 | Callable subtyping, ParamSpec |
| `narrowing_*` (2 failing) | 2 | TypeGuard, TypeIs narrowing |
| `protocols_*` (9 failing) | 9 | Structural subtyping |
| `aliases_*` (7 failing) | 7 | Type alias resolution, recursive aliases |
| `constructors_*` (5 failing) | 5 | Constructor call type resolution |
| `namedtuples_*` (4 failing) | 4 | NamedTuple type checking |
| `tuples_*` (2 failing) | 2 | Tuple type compatibility |

---

## Concrete Work Items

### WI-1: Final Annotation Deep Enforcement (E0054 expansion)
**Target files**: `qualifiers_final_annotation.py` (20 missed)
**New errors to detect**:
- `Final` without initializer in class body
- `Final` assignment outside `__init__`
- Reassignment of `Final` variable (local scope: `+=`, walrus `:=`, for-loop, with-statement, tuple unpack)
- `Final` in function parameter position
- `Final` nested inside container types (`list[Final[int]]`)
- `ClassVar[Final]` and `Final[ClassVar]` both illegal
- Module-level `Final` redefinition across multiple statements
**Resolver work**: Track all `Final`-annotated names and every subsequent write to those names
**Estimated gain**: 1 file (20 errors is a lot but they're all the same pattern)

### WI-2: @deprecated Usage Detection (new rule E0055)
**Target files**: `directives_deprecated.py` (12 missed)
**New errors to detect**:
- Call to `@deprecated` function
- Use of `@deprecated` class (instantiation, subclassing)
- Call to `@deprecated` method (including `__add__`, `__call__`)
- Access to `@deprecated` property (get and set)
- Import of `@deprecated` symbol from another module
**Resolver work**: Track `@deprecated` decorator on functions/classes/methods in resolved module, then flag call sites
**Estimated gain**: 1 file

### WI-3: ReadOnly TypedDict Mutation (new rule E0056)
**Target files**: `typeddicts_readonly.py` (6), `typeddicts_readonly_kwargs.py` (1), `typeddicts_readonly_update.py` (1)
**New errors to detect**:
- Direct assignment to ReadOnly field: `td["key"] = val`
- `.update()` call on TypedDict with ReadOnly fields
- `**kwargs` mutation of ReadOnly fields
**Resolver work**: Track `ReadOnly` annotation on TypedDict fields, detect subscript assignment and method calls
**Estimated gain**: 3 files

### WI-4: Frozen Dataclass Inheritance Conflict (extend E0052)
**Target files**: `dataclasses_frozen.py` (2 tagged groups)
**New errors to detect**:
- Non-frozen dataclass inheriting from frozen (or vice versa) — tagged error groups `E[DC2]`, `E[DC4]`
**Resolver work**: Check `frozen=` argument in `@dataclass` decorator across inheritance chain
**Estimated gain**: 1 file

### WI-5: Dataclass kw_only Enforcement (new rule E0057)
**Target files**: `dataclasses_kwonly.py` (3)
**New errors to detect**:
- Positional arguments passed to `kw_only=True` dataclass
**Resolver work**: Track `kw_only` on dataclass and individual fields, validate call sites
**Estimated gain**: 1 file

### WI-6: Dataclass __post_init__ Validation (new rule E0058)
**Target files**: `dataclasses_postinit.py` (4)
**New errors to detect**:
- `__post_init__` parameter count/types don't match `InitVar` fields
- Attribute access on `InitVar` field (not available at runtime)
**Resolver work**: Correlate `InitVar` fields with `__post_init__` signature
**Estimated gain**: 1 file

### WI-7: Dataclass slots=True Enforcement (new rule E0059)
**Target files**: `dataclasses_slots.py` (4)
**New errors to detect**:
- Assignment to undeclared attribute when `slots=True`
- Access to `__slots__` when `slots=False`
**Resolver work**: Track `slots=` argument, validate attribute assignments in methods
**Estimated gain**: 1 file

### WI-8: Dataclass match_args / order Enforcement (new rules E0060, E0061)
**Target files**: `dataclasses_match_args.py` (1), `dataclasses_order.py` (1)
**New errors to detect**:
- Access to `__match_args__` when `match_args=False`
- Use of `<`/`>`/`<=`/`>=` when `order=False`
**Resolver work**: Track decorator arguments, detect attribute access and comparison operators
**Estimated gain**: 2 files

### WI-9: Dataclass Final Field Assignment (extend E0054)
**Target files**: `dataclasses_final.py` (5)
**New errors to detect**:
- Assignment to `Final` field on dataclass instance or class
**Resolver work**: Already handled by Final enforcement (WI-1), just needs to work on dataclass fields too
**Estimated gain**: 1 file

### WI-10: Annotated Form Validation (extend E0045 + new checks)
**Target files**: `qualifiers_annotated.py` (8 missed, 1 false positive)
**New errors to detect**:
- Invalid first argument to `Annotated` (list, tuple, dict, lambda, f-string, conditional, bool literal, int literal, comprehension, subscript, `or` expression, variable)
- `Annotated` with fewer than 2 arguments
- `Annotated()` instantiation
- `Annotated[int, ""]()` instantiation
**Resolver work**: Validate AST node type of first argument to `Annotated[...]`
**Estimated gain**: 1 file

### WI-11: ClassVar Validation (extend E0036)
**Target files**: `classes_classvar.py` (12 missed)
**New errors to detect**:
- `ClassVar` with too many type arguments
- `ClassVar` containing TypeVar/ParamSpec/TypeVarTuple
- `ClassVar` in function parameter, local variable, return type, self attribute
- `Final[ClassVar[...]]` nesting
- `list[ClassVar[int]]` nesting
- `ClassVar` in TypeAlias
- Instance access to ClassVar attribute
**Resolver work**: Extend ClassVar context checking to cover more positions
**Estimated gain**: 1 file (need all 12, which is ambitious but all are structural)

### WI-12: TypedDict Operations (extend existing rules)
**Target files**: `typeddicts_operations.py` (11 missed)
**New errors to detect**:
- Wrong type assigned to known key
- Unknown key in subscript assignment or access
- Missing required key in dict literal
- Extra key in dict literal
- Variable key in TypedDict literal
- `.clear()` on TypedDict
- `del` on required key
**Resolver work**: TypedDict field tracking + subscript/literal validation
**Estimated gain**: 1 file (need all 11 — ambitious)

### WI-13: TypedDict Usage Validation (extend existing rules)
**Target files**: `typeddicts_usage.py` (5 missed)
**New errors to detect**:
- Invalid key assignment, wrong value type, `isinstance` with TypedDict, TypeVar bound to TypedDict
**Estimated gain**: 1 file

### WI-14: annotations_typeexpr.py (2 missed)
**Target files**: `annotations_typeexpr.py`
**What's needed**: Detect 2 invalid type expression forms
**Estimated gain**: 1 file

### WI-15: Historical Positional-Only Parameters
**Target files**: `historical_positional.py` (4 missed)
**What's needed**: Enforce positional-only parameter rules (PEP 570)
**Estimated gain**: 1 file

---

## Score Projection

| Work Item | Files Gained | Running Total | Score |
|---|---|---|---|
| Baseline | 0 | 41 | 28.3% |
| WI-1: Final deep enforcement | 1 | 42 | 29.0% |
| WI-2: @deprecated detection | 1 | 43 | 29.7% |
| WI-3: ReadOnly TypedDict mutation | 3 | 46 | 31.7% |
| WI-4: Frozen dataclass inheritance | 1 | 47 | 32.4% |
| WI-5: Dataclass kw_only | 1 | 48 | 33.1% |
| WI-6: Dataclass __post_init__ | 1 | 49 | 33.8% |
| WI-7: Dataclass slots | 1 | 50 | 34.5% |
| WI-8: Dataclass match_args + order | 2 | 52 | 35.9% |
| WI-9: Dataclass Final fields | 1 | 53 | 36.6% |
| WI-10: Annotated form validation | 1 | 54 | 37.2% |
| WI-11: ClassVar validation | 1 | 55 | 37.9% |
| WI-12: TypedDict operations | 1 | 56 | 38.6% |
| WI-13: TypedDict usage | 1 | 57 | 39.3% |
| WI-14: annotations_typeexpr | 1 | 58 | 40.0% |
| WI-15: historical_positional | 1 | 59 | 40.7% |
| **Subtotal (structural only)** | **18** | **59** | **40.7%** |

### The Gap: 40.7% → 50%

The structural work gets us to ~41%. We need **14 more files** to hit 50%. These must come from categories that are *partially* structural but have some inference-dependent errors. Options:

#### Option A: Lightweight Type Comparison (not full inference)

Several files need simple type comparison — not TypeVar solving, just "does `int` match `str`?". We already do this in E0012/E0014. Extending this to cover:

| File | Missed | What Simple Comparison Catches |
|---|---|---|
| `overloads_basic.py` | 1 | No matching overload (literal arg vs overload params) |
| `overloads_evaluation.py` | 2 | No matching overload + literal type mismatch |
| `enums_member_values.py` | 2 | Wrong value type for typed enum member |
| `dataclasses_hash.py` | 2 | Unhashable type assigned to Hashable |
| `enums_expansion.py` | 1 | assert_type mismatch |
| `enums_members.py` | 6 | Annotated member + Literal[EnumMember] validation |
| `directives_assert_type.py` | 5 | 3 structural + 2 type comparison |
| `typeddicts_readonly_consistency.py` | 7 | ReadOnly vs mutable assignment compat |
| `typeddicts_readonly_inheritance.py` | 10 | ReadOnly inheritance + Required conflicts |
| `typeddicts_extra_items.py` | 23 | closed TypedDict validation, extra_items |
| `typeddicts_type_consistency.py` | 9 | TypedDict structural compatibility |

**If we add lightweight type comparison** (comparing declared annotations, not solving constraints), we could pick up another 8–12 files from this list, easily clearing 50%.

#### Option B: Reduce False Positives on Passing-Threshold Files

Some files might be failing because our strictness rules (E0001–E0005) fire false positives on lines without `# E` markers. The conformance harness already excludes these from scoring, but double-check that the exclusion is working correctly. A fix here could flip files for free.

#### Option C: Partial Credit via Tagged Groups

Some files use `# E[tag]` groups where only ONE line in the group needs a diagnostic. If we already catch one line in the group, the whole group passes. Check if our existing rules accidentally satisfy any tagged groups we're not counting.

### Recommended Path to 50%

1. **Do all 15 structural work items** (Tier 1–3) → 40.7%
2. **Add lightweight type comparison for TypedDict and overloads** → pick up `typeddicts_readonly_consistency.py`, `typeddicts_readonly_inheritance.py`, `typeddicts_extra_items.py`, `overloads_basic.py`, `overloads_evaluation.py`, `directives_assert_type.py` → ~46.9%
3. **Extend enum validation** → pick up `enums_member_values.py`, `enums_members.py` → ~48.3%
4. **Handle dataclass_transform basics** → pick up `dataclasses_transform_field.py` (only 2 missed) → ~49.0%
5. **One more from TypedDict** → `typeddicts_type_consistency.py` → **50.3%**

---

## Execution Order (by bang-for-buck)

**Week 1 — Quick wins (8 files)**
1. WI-3: ReadOnly TypedDict mutation (3 files, similar pattern)
2. WI-8: Dataclass match_args + order (2 files, trivial)
3. WI-4: Frozen dataclass inheritance (1 file, small)
4. WI-5: Dataclass kw_only (1 file, small)
5. WI-14: annotations_typeexpr (1 file, small)

**Week 2 — Dataclass completion (4 files)**
6. WI-6: Dataclass __post_init__ (1 file)
7. WI-7: Dataclass slots (1 file)
8. WI-9: Dataclass Final fields (1 file)
9. WI-10: Annotated form validation (1 file)

**Week 3 — Big structural rules (3 files)**
10. WI-1: Final deep enforcement (1 file, 20 errors but uniform pattern)
11. WI-2: @deprecated detection (1 file, needs cross-module resolver work)
12. WI-11: ClassVar validation (1 file)

**Week 4 — TypedDict + remaining (3 files)**
13. WI-12: TypedDict operations (1 file)
14. WI-13: TypedDict usage (1 file)
15. WI-15: historical_positional (1 file)

**Week 5–6 — Lightweight type comparison bridge (14 files to 50%)**
16. Extend type comparison to TypedDict assignment compatibility
17. Extend type comparison to overload dispatch
18. Extend type comparison to enum member values
19. Extend type comparison to assert_type

---

## What This Does NOT Cover

- **No TypeVar solving** — generics stay at 4/30
- **No narrowing** — stays at 0/2
- **No protocol satisfaction** — stays at 2/11
- **No callable subtyping** — stays at 0/4
- **No type alias resolution** — stays at 0/7
- **No constructor call typing** — stays at 1/6

These are all Phase 2+ (type inference engine). This plan is exclusively about extracting maximum value from structural/syntactic/resolver-level analysis.

---

## Risk Factors

1. **Some files need ALL errors caught** — missing even one `# E` line fails the whole file. The estimates assume we can catch every required error. In practice, some files may have 1–2 errors that need inference we didn't account for.

2. **Cross-module resolution** — `@deprecated` detection requires reading imported modules. The resolver may need enhancement to track decorators across imports.

3. **False positive management** — adding new rules must not introduce false positives on currently-passing files. Every new rule needs a guard for conformance contexts.

4. **Tagged groups** — `# E[tag]` semantics require that we fire on the RIGHT line, not just any line in the file. Incorrect line targeting fails the group.

**Mitigation**: Run the conformance suite after every work item. Never merge a rule that causes a regression.
