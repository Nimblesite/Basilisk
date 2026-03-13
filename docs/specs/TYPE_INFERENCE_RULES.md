# Type Inference Rules — Implementation Plan

## The Core Problem

The type inference engine (`inference.rs`, `collection_inference.rs`, `types.rs`) is
built and passes its own tests. But the **coverage data reveals the truth**:

| Module | Line Coverage | Why |
|---|---|---|
| `collection_inference.rs` | **0.00%** | Never called from any rule |
| `types.rs` | 81.56% | Exercised only by unit tests inside the file |
| `inference.rs` | 86.36% | Exercised only by unit tests inside the file |

Zero rules import `use crate::inference`. The `check()` function calls
`rules::run_all(module)` — no inference involved. Basilisk currently detects type
errors through `RhsKind` pattern matching, not through the `InferredType` system.

### The Tests Are Misleading

`inference_tests.rs` and `collection_inference_tests.rs` are **unit tests** that call
`infer_rhs()`, `infer_list_type()`, etc. directly. They pass because the functions
work in isolation. They do NOT prove the checker uses inference on real Python input.

**These tests must be replaced** with E2E coarse tests that submit real Python source
through the full pipeline and assert that specific diagnostics fire (or do not).

---

## Goal

Wire the existing inference engine into the checker pipeline and implement the rules
that surface type inference as a visible feature — especially **W0050 (redundant
annotation warning)**, which is a core differentiator from Pyright, Pyrefly, and ty.

---

## Step 0 — Fix the Misleading Tests (PREREQUISITE)

**Files**: `crates/basilisk-checker/tests/inference_tests.rs`,
`crates/basilisk-checker/tests/collection_inference_tests.rs`

Every test must be rewritten using the `run()` helper pattern from `checker_tests.rs`.
Each test submits real Python source and asserts on diagnostic output.

Example of a correct E2E inference test:
```rust
#[test]
fn test_infer_detects_type_mismatch() {
    let diags = run("x: str = 42\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-E0014"));
}
```

The current tests that call `infer_rhs()` and `FlowUnionTracker` directly are NOT
coarse tests and must be replaced. Until `collection_inference.rs` shows >0% coverage
from the pipeline, inference is not wired in.

---

## Step 1 — Connect Inference to E0014 (Assignment Type Mismatch)

**File**: `crates/basilisk-checker/src/rules/e0014.rs`
**Current coverage**: 82.50% regions — the uncovered 18% is the inference path

For every `VariableInfo` with `has_annotation == true`:
1. Call `infer_rhs(&var_info.rhs_kind)` → `InferredType`
2. Parse the annotation text to an `InferredType`
3. Call `inferred.is_assignable_to(&declared)` — if false, emit E0014

`InferredType::is_assignable_to()` already exists in `types.rs`. This replaces the
current `RhsKind` pattern matching with proper type comparison.

```python
x: str = 42       # E0014 — int not assignable to str
x: int = "hello"  # E0014 — str not assignable to int
x: float = 42     # NO error — int is assignable to float (widening)
x: bool = 1       # E0014 — int not assignable to bool (bool is subtype of int, not reverse)
```

**Coverage target**: `e0014.rs` from 82.50% → 95%+

---

## Step 2 — W0050: Redundant Annotation Warning (KEY FEATURE)

**File to create**: `crates/basilisk-checker/src/rules/w0050.rs` (does not exist)
**Wire into**: `crates/basilisk-checker/src/rules/mod.rs`

**This is Basilisk's headline differentiator.** No competing tool warns when a type
annotation is redundant. Basilisk's position: if the type system can infer it
precisely, writing the annotation is noise.

**Rule**: When `has_annotation == true` AND inferred type exactly matches the declared
annotation, emit `BSK-W0050: redundant type annotation — inferred type is identical; remove the annotation`.

```python
# W0050 fires — annotation is redundant
x: int = 42
y: str = "hello"
z: float = 3.14
items: list[int] = [1, 2, 3]
pairs: dict[str, int] = {"a": 1}

# W0050 does NOT fire — annotation adds information
x: float = 42             # widens int → float
items: list[int|str] = [1]   # widens list[int] → list[int|str]
coords: tuple[float, float] = (0, 0)  # widens tuple[int, int]
```

**Algorithm**:
1. `inferred = infer_rhs(&var_info.rhs_kind)` — must not be `Unknown`
2. Parse `var_info.annotation_text` → `declared: InferredType`
3. If `inferred == declared` (exact equality) → emit W0050

**Scope where W0050 fires**:
- Local variable assignments
- Module-level variable assignments
- Class body variable assignments
- For-loop targets, with-statement targets, walrus operator targets

**Scope where W0050 NEVER fires** (annotations always required here):
- Function parameters — E0001 territory
- Public function return types — E0002 territory
- `TypedDict` fields, `NamedTuple` fields, `Protocol` members
- `ClassVar` and `Final` qualifiers

**Tests** (all E2E, all in `inference_tests.rs`):

| Python | Expected |
|---|---|
| `x: int = 42` | W0050 fires |
| `x: str = "hello"` | W0050 fires |
| `x: float = 3.14` | W0050 fires |
| `x: list[int] = [1, 2, 3]` | W0050 fires |
| `x: float = 42` | NO diagnostic |
| `x: list[int\|str] = [1]` | NO diagnostic |
| `def f(x: int): ...` | NO W0050 (params exempt) |

---

## Step 3 — Collection Inference Wiring (0% → 80%+)

**File**: `crates/basilisk-checker/src/collection_inference.rs`
**Current coverage**: **0.00%** — must become >80%

`collection_inference.rs` implements `infer_list_type`, `infer_dict_type`,
`infer_set_type`, `infer_tuple_type`. These are called from `infer_rhs()` but
`infer_rhs()` itself is never called from any rule.

Wiring Steps 1 and 2 above (E0014 and W0050) will automatically drive
`collection_inference.rs` coverage up because `infer_rhs()` delegates to it for
`RhsKind::List`, `RhsKind::Dict`, `RhsKind::Set`, `RhsKind::Tuple`.

No separate work item needed — coverage rises as a side effect of Step 1 + Step 2.

**Verification gate**: After Steps 1–2 land, `collection_inference.rs` line coverage
must be >80%. If it's still near 0%, the wiring is incomplete.

---

## Step 4 — E0011: Explicit Any Annotation (inference-backed)

**File**: `crates/basilisk-checker/src/rules/e0011.rs`
**Current coverage**: 90.28% regions — the gap is the inference-backed path

E0011 and W0050 are complementary:
- W0050 fires when annotation exactly matches inference (redundant)
- E0011 fires when annotation is `Any` but inference has a concrete type

```python
x: Any = 42      # E0011 — inference gives int; explicit Any is lazy
x: Any = foo()   # NO E0011 — RHS is a call, inference gives Unknown; Any is legitimate
```

Algorithm: if annotation is `Any` AND `infer_rhs()` returns a non-Unknown concrete
type → emit E0011 (the programmer could have written the real type).

**Coverage target**: `e0011.rs` from 90.28% → 97%+

---

## Step 5 — Return Type Inference (E0013)

**File**: `crates/basilisk-checker/src/rules/e0013.rs`
**Current coverage**: 100% — but the inference-backed return-union check is missing

Per `TYPE_INFERENCE.md §4.3`, the inferred return type is the union of all `return`
expression types. If the declared return type is narrower than the inferred union,
emit E0013.

```python
def f(x: int) -> int:   # E0013 — inferred: int | str, declared: int
    if x > 0:
        return x
    return "negative"
```

**Prerequisite**: `FunctionInfo` in `basilisk-resolver` must expose a list of return
expression `RhsKind`s. Check whether this is already surfaced before implementing.
If not, this is a resolver extension task first.

---

## Coverage Targets

After all steps complete:

| Module | Before | After |
|---|---|---|
| `collection_inference.rs` | 0.00% | >80% |
| `inference.rs` | 86.36% | >95% |
| `types.rs` | 81.56% | >90% |
| `e0014.rs` | 82.50% | >95% |
| `e0011.rs` | 90.28% | >97% |
| `w0050.rs` | (new) | >95% |

### Low-Coverage Rules Requiring Separate Attention

These are not inference-related but must be tracked and fixed:

| Rule | Line Coverage | Action |
|---|---|---|
| `e0057.rs` | **16.92%** | Critical — add E2E tests immediately |
| `e0072.rs` | 74.43% | Add E2E tests for uncovered branches |
| `e0095.rs` | 85.07% | Add E2E tests for uncovered branches |
| `e0096.rs` | 89.36% | Add E2E tests for uncovered branches |
| `e0076.rs` | 85.76% | Add E2E tests for uncovered branches |

---

## Priority Order

| # | Task | File | Unblocks |
|---|---|---|---|
| 1 | Rewrite misleading tests as E2E | `inference_tests.rs`, `collection_inference_tests.rs` | Everything |
| 2 | Wire E0014 to use `infer_rhs` + `is_assignable_to` | `e0014.rs` | W0050 (same machinery) |
| 3 | Implement W0050 | `w0050.rs` (new) | Headline feature |
| 4 | Extend E0011 with inference check | `e0011.rs` | Completeness |
| 5 | Wire return-type inference into E0013 | `e0013.rs` | Requires resolver check first |
| 6 | Fix `e0057.rs` coverage (16% → 80%+) | `e0057_tests.rs` | Coverage health |

---

## Verification

After each step:
1. `cargo build` — clean
2. `cargo clippy --all-targets --all-features -- -D warnings` — zero warnings
3. `cargo test -p basilisk-checker --test inference_tests` — all E2E tests pass
4. `cargo test --workspace` — zero failures
5. Check coverage: `collection_inference.rs` must show >0% to confirm wiring is real

**The headline gate**: `x: int = 42` submitted through the full pipeline must produce
a BSK-W0050 diagnostic. Until that test passes end-to-end, type inference is not wired in.
