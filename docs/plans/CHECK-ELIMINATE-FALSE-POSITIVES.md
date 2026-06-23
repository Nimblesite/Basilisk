# Plan: Eliminate False Positives in PEP Conformance Suite

> ⚠️ **SUPERSEDED.** The numbers in this doc ("136/146 PASS / 93.15%", "170
> false positives", "FP-ceiling … Set to 161", `diag_line_rules`,
> `missed == 0` pass rule) describe an earlier in-repo harness that has been
> **removed**. The score is now computed by the **real `python/typing`
> calculator** (`conformance/score.py` downloads and runs upstream's own
> `get_expected_errors` + `diff_expected_errors`; see [CHKARCH-CONFORMANCE]).
> A file passes only with an **empty upstream `errors_diff`** (false positives
> fail the file), and **no diagnostic codes are excluded**. Honest current
> baseline: **59/146 = 40.4%**, **285 false positives**, 36 missed. The
> still-valid part of this plan is the *strategy* — driving specific rules'
> false positives down; the *counts* below are stale.

## Context

False positives are diagnostics Basilisk reports on lines that have NO `# E`
annotation — the typing spec says the line is **valid code** but Basilisk flags
it anyway. They erode user trust and block adoption.

The conformance harness (`crates/basilisk-cli/tests/conformance_tests.rs`) now
prints per-FP verbose output (`FP <file>: count=N lines=[(line, rule)…]`, see
`diag_line_rules`). Run `cargo test --test conformance_tests --release -- --nocapture`
and `grep '  FP    '` to get the exact rule→line mapping.

### CURRENT STATE (measured 2026-06-03, on `main` after PR #73)

- **136/146 PASS (93.15%)**, threshold pinned at 93 in `coverage-thresholds.json`.
- **170 false positives** across 50 files. (The earlier "18 FPs" claim in this
  doc's history was a never-merged aspiration — the real number is 170.)

> **HARD INVARIANT.** Conformance is monotonic: PASS count only goes UP, FP count
> only goes DOWN. A file PASSES iff `missed == 0`. Flipping ONE file PASS→FAIL
> drops us to 92.46% < 93 → CI fails. **Every FP fix must reduce FPs with ZERO
> new missed diagnostics.** Verify empirically: after each change re-run the
> harness and diff `conformance_status.csv` against baseline — no file may regress
> PASS→FAIL and total `missed` must not increase.

### FP distribution by rule (the real target list)

| Rule | FPs | What it is | Dominant pattern |
|------|-----|------------|------------------|
| **E0014** | **~105** | assignment type mismatch | `local: NamedType = param` — param-type-map lookup yields a `Named`/`Callable` type that the `_ => false` catch-all in `is_assignable_to` rejects. E0014 is a *literal-mismatch* checker; protocol/callable/alias assignability belongs to E0136/E0121/E0099. |
| E0053 | 15 | `assert_type` equivalence | text-based type equivalence misses spec-equal forms |
| E0111 | 9 | constructor calls | over-strict arg checking on transformed/namedtuple/erased classes |
| E0093 | 7 | TypedDict | `extra_items`/`ReadOnly`/`Final` field semantics |
| E0013 | 7 | return type mismatch | base↔Literal and structural return widening |
| E0012 | 4 | TypeVarTuple unpack | — |
| E0130 / E0047 | 3 each | scoping / forward refs | — |
| ~15 rules | 1–2 each | scattered | per-rule edge cases |

---

## Strategy (this PR)

Fix the rules in descending FP order, **verifying empirically after each** against
the saved baseline (`/tmp/conf_baseline.csv`). Group the E0014 mass into surgical,
TP-safe guards rather than rewriting the (text-based) rule wholesale.

- **FIX A — E0014 param-lookup conservatism** (`e0014/mod.rs` param branch). Only
  adopt a parameter/variable's inferred type when *both* it and the declared type
  are **value-adjudicable** (Int/Str/Float/Bool/Bytes/None/Literal/builtin
  containers thereof) — never `Named`/`Callable`/`TypeForm`. Preserves the
  `Literal[False] = a` (a: Literal[0]) TP while killing every `local: Named = param`
  FP. Target ≈ 56 FPs (callables_annotation, callables_subtyping, protocols_*,
  specialtypes_*, typeddicts_readonly_consistency).
- **FIX B — E0014 TypedDict skip broadening**: when the declared type is a TypedDict,
  skip for *all* RHS shapes (not just dict literals); E0093 owns TD field checking.
- **FIX C — directives**: recognise `# type: ignore[code]` bracket form, file-level
  `# type: ignore` before docstring, and suppress assignments under
  `if not TYPE_CHECKING:` (3 FPs).
- **FIX D — E0053** (15 FPs), **FIX E — E0013** (7 FPs), **FIX F — E0111** (9 FPs):
  independent rules; root-cause + TP-safe patch per cluster.

### Enforcement upgrade

Add `conformance.max_false_positives` to `coverage-thresholds.json` and assert it
in the harness (ratchet DOWN only), making FP a true quality gate alongside the
PASS-percentage gate — "quality metrics only increase per PR".

---

## Step 0: Add FP Verbose Reporting to Conformance Harness

**File**: `crates/basilisk-cli/tests/conformance_tests.rs`

Add a debug print (like the existing missed-lines print at line 284) that shows:
- Which lines are FPs (diagnostic on unannotated line)
- Which rule code fired on each FP line

This gives us the exact rule-to-line mapping needed to fix each FP surgically. Currently we only know FP counts per file, not which rules cause them.

```rust
// After line 275, add:
if false_positives > 0 {
    let fp_details: Vec<(usize, String)> = /* collect (line, rule) for FP lines */;
    println!("  FP {file_name}: count={false_positives} lines={fp_details:?}");
}
```

This requires threading the rule code through with the line number (currently `diag_lines` is `HashSet<usize>` — change to store `(line, rule_code)` pairs).

**Verification**: Run `cargo test --test conformance_tests -- --nocapture` and inspect FP output.

---

## Step 1: E0104 — Recursive Type Aliases (est. ~20 FP eliminated)

**File**: `crates/basilisk-checker/src/rules/e0104.rs`
**Conformance file**: `aliases_recursive.py` (20 FP)

**Problem**: E0104 flags ALL cyclical type aliases, but PEP 695 explicitly allows recursive aliases that have a terminating base case (e.g., `Json = Union[None, int, str, list["Json"]]`). Only truly infinite cycles should be flagged (e.g., `RecursiveUnion: TypeAlias = Union["RecursiveUnion", int]` where the recursive branch wraps nothing new).

**Fix**:
- Check if the alias is a PEP 695 `type` statement → always allow recursion
- For `TypeAlias`-annotated assignments, check if the Union has at least one non-recursive branch that doesn't reference any alias in the cycle → allow (it's a valid recursive type with a base case)
- Only flag aliases where the recursion cannot terminate (all paths lead back to self with no widening)

The test file shows exactly which should error: only lines 72 and 75 (`RecursiveUnion` and `MutualReference1/2`). Lines 14, 24, 30, 42, 58, 65 are valid recursive aliases.

---

## Step 2: E0136 — Callable Subtyping (est. ~25 FP eliminated)

**File**: `crates/basilisk-checker/src/rules/e0136.rs`
**Conformance file**: `callables_subtyping.py` (25 FP)

**Problem**: The `is_subtype` function uses text-based type comparison and only handles the numeric hierarchy (`int`/`float`/`complex`/`bool`) and exact name matches. It doesn't understand:
- `object` as universal supertype for all parameters
- Union types in parameters (contravariance: `Callable[[int|str], None]` accepts `Callable[[int], None]`)
- `*args`/`**kwargs` parameter compatibility
- Default parameter arity tolerance
- Protocol `__call__` ↔ `Callable` equivalence

**Fix**: Enhance `is_subtype` to handle Union decomposition, `object`, and `None`/`Optional`. Add `*args`/`**kwargs` awareness to parameter count checking.

---

## Step 3: E0014 — Assignment Type Mismatch (est. ~30 FP eliminated)

**Files**: `crates/basilisk-checker/src/rules/e0014/mod.rs`, `literal_parse.rs`, `tuple_check.rs`
**Top conformance files**: `callables_annotation.py` (17 FP), `tuples_type_compat.py` (17 FP), `specialtypes_any.py` (7 FP), `typeddicts_readonly_consistency.py` (5 FP)

**Problem**: E0014 uses text-based annotation parsing and literal inference. It over-reports when:
- Annotation is a `Callable[...]` or Protocol name (should defer to E0136/E0140)
- Annotation is a bare generic (`list`, `dict` without `[...]`) which implicitly means `Any`
- Annotation is a recursive type alias name
- Annotation involves `tuple[T, ...]` homogeneous form
- RHS is a function call, parameter reference, or complex expression (not a simple literal)

**Fix**: Add suppression guards:
1. Skip when annotation text references a known type alias from `module.type_alias_defs`
2. Skip when annotation starts with `Callable` (defer to E0136/E0140)
3. Skip when annotation is a bare generic name without `[...]`
4. Improve tuple annotation handling for variadic forms

---

## Step 4: E0093 + TypedDict Rules (est. ~15 FP eliminated)

**Files**: `crates/basilisk-checker/src/rules/e0093/mod.rs`, `type_consistency.rs`
**Conformance files**: `typeddicts_extra_items.py` (13 FP), `typeddicts_readonly.py` (2 FP), `typeddicts_required.py` (3 FP), `typeddicts_readonly_consistency.py` (5 FP), `typeddicts_readonly_update.py` (3 FP)

**Problem**: TypedDict checking has gaps around:
- `extra_items` (PEP 728) — TypedDicts can allow extra items with a declared type
- `ReadOnly` fields (PEP 705) — read operations on ReadOnly fields are valid
- TypedDict-to-TypedDict structural assignment compatibility
- Inheritance field resolution (transitive required/optional status)

**Fix**:
- Add `extra_items` support to E0093's key validation
- Handle ReadOnly semantics (reads are always valid, only mutation blocked)
- Build transitive field resolution for TD inheritance chains

---

## Step 5: Protocol Rules — E0121, E0110, E0133 (est. ~25 FP eliminated)

**Files**: `crates/basilisk-checker/src/rules/e0121.rs`, `e0110.rs`, `e0133.rs`
**Conformance files**: `protocols_definition.py` (13 FP), `protocols_subtyping.py` (7 FP), `protocols_merging.py` (2 FP), `protocols_recursive.py` (2 FP), `callables_protocol.py` (1 FP)

**Problem**:
- E0121: Protocol conformance checking misses inherited methods, builtin type dunders, and attribute-only protocol members
- E0110/E0133: Variance inference doesn't exempt `__init_subclass__`, `@property`, or explicit `self: X[T]` annotations

**Fix**:
- E0121: Walk full MRO for method resolution, add builtin dunder table, check attributes
- E0110: Add more exempt methods, detect `@property` as output-only
- E0133: Handle PEP 695 inline TypeVar declarations

---

## Step 6: Remaining Scatter (est. ~20 FP eliminated)

**Rules**: E0013, E0036, E0053, E0061, E0045, E0147, and other low-count rules
**Files**: `exceptions_context_managers.py` (6 FP), `enums_members.py` (7 FP), `narrowing_typeguard.py` (5 FP), `narrowing_typeis.py` (4 FP), `literals_interactions.py` (4 FP), many files with 1-3 FP

These require per-rule investigation after Step 0 gives us exact FP-to-rule mapping. Common themes:
- Context manager `__exit__` return type narrowing
- Enum member/nonmember decorator semantics
- TypeGuard/TypeIs narrowing edge cases
- Literal type interactions

---

## Step 7: Type Narrowing and Full Inference Engine (est. ~125 FP eliminated)

> **Full plan**: [CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md](CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md)
> **Spec**: [CHECKER-TYPE-INFERENCE-SPEC.md §TYPEINF-NARROWING](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING)

The remaining ~125 FPs cannot be fixed without fundamental engine work. The checker currently uses text-based annotation parsing and literal-only RHS inference. It has no narrowing engine, no TypeVar constraint solver, and no class hierarchy resolution. See the dedicated plan for full details.

---

## Execution log

### 2026-06-03 (session 2) — FP reduction 161 → 136 and counting (zero conformance regression)

Verification harness: [`scripts/fp_verify.sh`](../../scripts/fp_verify.sh) rebuilds, runs
the conformance suite, and diffs `conformance_status.csv` against a saved baseline,
flagging any PASS→FAIL flip, `caught` drop, or `missed` rise plus the per-file FP delta.
Run after **every** change; revert anything that regresses. Baseline advances only to
verified-better states.

Throughout: **PASS=136, caught=858, missed=95 unchanged**; only `fp` moves (down).

| # | Change | Files | FPs | Status |
|---|--------|-------|-----|--------|
| 1 | Bare `Callable` → `Callable[..., Any]`, bare `type` → `type[Any]` (≈Any); `tuple[()]` → empty tuple; `None` assignable to `Hashable`; skip whole-quoted annotations in E0014; E0099 skips type-utility (`assert_type`/`reveal_type`/`cast`) args | types_parsing.rs, types.rs, e0014/mod.rs, protocol_ext.rs | **−8** | DONE (161→153) |
| 2 | `tuple[Any,...]` / `tuple[Unknown,...]` source assignable to fixed-length target (PEP 484 gradual) | types.rs | **−3** | DONE (153→150) |
| 3 | Recursive **value-alias** matcher: resolve bare `Name = Union[...]` aliases (Json/RecursiveTuple/RecursiveMapping) and positively match the inferred literal against the expanded (recursive) definition. Positive-match semantics keep `Unknown` (e.g. `3j`) from matching, so the `# E` lines still fire | e0014/alias_match.rs (new), e0014/mod.rs | **−14** | DONE (150→136) |

**Why the alias matcher is TP-safe:** it only *suppresses* when the value demonstrably
matches the alias; `Unknown`/`Any` never positively match a concrete target, so the
fixtures' incompatible assignments (`3j`, stray `list`s) keep firing. Restricted to
`Union`-bodied aliases referenced by a bare name → excludes the 4 generic-alias FPs
(`GenericTypeAlias1/2`, which need TypeVar substitution — see Deferred).

**Deferred this session (need engine/resolver work, out of scope for a no-regression PR):**

- **Callable/protocol structural subtyping** (callables_subtyping 24, callables_annotation 16,
  protocols_subtyping 7, protocols_merging 2 ≈ **49 FPs**). These are *load-bearing*: E0014's
  `Named`-base-mismatch catches both the OK and the `# E` lines (mirror-image assignments like
  `f3: PosOnly2 = standard` OK vs `f1: Standard2 = pos_only` E). Eliminating the FPs requires
  real PEP 484 callable subtyping with **parameter-kind awareness** (positional-only `/`,
  keyword-only `*`, `*args`/`**kwargs` supertype rules, defaults, overloads, ParamSpec). Blocked
  on a prerequisite: `ParameterInfo` does not capture parameter kind today — a resolver expansion
  is needed first. Suppressing without it would drop ~40 TPs and slip conformance.
- **Variadic-tuple star matching** (tuples_type_compat 11): PEP 646 `*tuple[...]` prefix/middle/suffix.
- **Recursive generic aliases** (aliases_recursive 4): TypeVar substitution.
- **TypedDict structural assignability** (E0014 readonly_*/inheritance 8, E0093 extra_items/ReadOnly 7).
- **`assert_type` narrowing** (E0053 15): flow narrowing (isinstance/`is`/TypeGuard/TypeIs) + Union/tuple-unpack syntactic equivalence.
- **Constructors** (E0111 9): dataclass_transform class/converter, NamedTuple subscript/inheritance, type-erasure.
- **Scattered singles** (E0012/E0048/E0054/E0069/E0060/E0115/E0130/E0094/E0078/E0092/E0139/E0112/E0140/E0041/E0143).

---

### 2026-06-03 (session 1) — clean FP reduction: 170 → 161 (zero conformance regression)

Measured against `main`. Every change verified empirically: re-ran the harness and
diffed `conformance_status.csv` for PASS→FAIL flips AND `caught`/`missed` deltas.

| Change | Rule | FPs removed | Result |
|--------|------|-------------|--------|
| `is_unverifiable_return_type` recursive skip (Named **and** Literal at any nesting) | E0013 | **7 (all of them)** | **DONE.** E0013 is now FP-free. Quoted forward-ref unions (`"int \| Meta2"`), `tuple[()]`, and `Literal[...]`/`Literal`-tuple returns are unverifiable by kind-only return inference; skipping them loses no TP (verified: the suite's only Literal-return sites are the OK cases fixed + `...`-body overload stubs + `LiteralString`). |
| File-level `# type: ignore` + spec-compliant `# type: ignore[<non-BSK>]` | suppression (E0014 FPs) | **2** | **DONE.** Standalone top-of-file `# type: ignore` (only blank/comment lines before) silences the file; bracketed non-Basilisk tags suppress all per PEP 484, while `[BSK-…]` stays code-specific. TP at `directives_type_ignore_file2.py:14` preserved (comment after docstring). |
| FP-ceiling gate | harness | 0 (enforcement) | **DONE.** `conformance.max_false_positives` in `coverage-thresholds.json`, asserted in `conformance_tests.rs`. Ratchets DOWN only — mirrors the pass-% gate. Set to **161**. |

**Rejected (would reduce `caught` — violates the monotonic invariant):**

- **E0014 param-lookup conservatism (FIX A)** — removed ~58 FPs but flipped 4 files
  PASS→FAIL (callables_annotation/subtyping, protocols_subtyping,
  typeddicts_readonly_consistency add +47 `missed`). In those files E0014 is the
  *de-facto partial subtyping checker* (it catches the `# E` lines via Named/Callable
  comparison while FP-ing on the OK ones). Reverted. This cluster needs **real
  callable/protocol subtyping** (Steps 2/5), not suppression.
- **E0014 transitive-TypedDict skip (FIX B)** — removed 5 FPs but dropped `caught`
  858→856 (+2 `missed` on the already-FAIL `typeddicts_extra_items` /
  `typeddicts_readonly_inheritance`): E0014's blanket dict→TD flag was *accidentally*
  catching `# E` lines that E0093 misses. Reverted. Needs E0093 field-level work.

**Deferred (needs engine work, out of scope for a no-regression PR):**

- **E0053 (assert_type, 15 FPs)** — 11 are flow narrowing (`isinstance`/`TypeGuard`/
  `is`-comparison); the rest are Union-syntax / unpacked-tuple equivalence in the
  resolver's `types_match` (high blast radius, medium confidence). Needs the
  narrowing engine + semantic type normalisation.
- **E0111 (constructor calls, 9 FPs)** — multiple distinct causes (generic-NamedTuple
  subscript, NamedTuple inheritance, `@dataclass_transform` frozen bases) across a
  FAIL file. Each is its own investigation; high regression risk in one pass.
- **E0014 mass (~96 remaining)** — callables/protocols/recursive-aliases/tuples all
  require real subtyping or value-level recursive-alias checking.

### Historical notes (pre-2026-06-03, partially superseded)

The status table below predates the measured baseline above; several "DONE (N FPs)"
claims describe branches that did not land on `main` (the real baseline was 170 FPs,
not the 18 this doc once claimed). Kept for the root-cause analysis it contains.

| Step | Rule | Notes |
|------|------|-------|
| 0 | FP verbose reporting | DONE (`diag_line_rules` in `conformance_tests.rs`) |
| 1 | E0104 recursive aliases | container-wrapped recursion allowed |
| 2 | E0136 callable subtyping | **still needed** (see Rejected/FIX A) |
| 5 | E0121/E0110/E0133 protocols | **still needed** (see Rejected/FIX A) |

---

## Verification

After each step:
1. Run `cargo test --test conformance_tests -- --nocapture`
2. Check that `conformance_status.csv` FP counts decrease
3. Verify no new test failures (missed count must not increase)
4. Run `cargo clippy` and `cargo fmt`

---

## TODO — live checklist (ratchets DOWN; tick as eliminated)

> **Total: 126 FPs** (this session: 161 → 126, −35, ZERO conformance regression).
> Gate: `coverage-thresholds.json` `max_false_positives = 126` (ratchet DOWN only).
> Each item verified via [`scripts/fp_verify.sh`](../../scripts/fp_verify.sh): no PASS→FAIL,
> `caught`≥858, `missed`≤95.

### Done this session (−35)

- [x] Bare `Callable` → `Callable[..., Any]`, bare `type` → Any (`types_parsing.rs`)
- [x] `tuple[()]` → empty tuple type (`types_parsing.rs`)
- [x] `None` assignable to `Hashable` (`types.rs`)
- [x] Skip whole-quoted forward-ref annotations in E0014 (`e0014/mod.rs`)
- [x] E0099 skips `assert_type`/`reveal_type`/`cast` arguments (`protocol_ext.rs`)
- [x] `tuple[Any,...]`/`tuple[Unknown,...]` source → fixed-length target (gradual) (`types.rs`)
- [x] Recursive `Union` value-alias matcher (Json/RecursiveTuple/RecursiveMapping) (`e0014/alias_match.rs`)
- [x] PEP 646 variadic-tuple `*tuple[...]` star matching (`types.rs`)
- [x] Non-decimal `Literal` integer equivalence (`0x14`==`0o24`==`0b10100`) (`types_parsing.rs`)
- [x] Shared `parse_key_value_args` helper (dedup `dict[]`/`Mapping[]` parsing)
- [x] Mutation-hardening e2e tests (`tests/fp_elimination_tests.rs`) + FP-ceiling ratchet → 126

### Remaining — deferred engine work (need narrowing / structural subtyping / resolver expansion)

- [ ] **Callable/protocol structural subtyping** (~49): callables_subtyping (24), callables_annotation (16), protocols_subtyping (7), protocols_merging (2). Blocked on resolver capturing parameter **kind** (positional-only `/`, keyword-only `*`) in `ParameterInfo`, then full PEP 484 callable subtyping. Load-bearing — suppression drops ~40 TPs.
- [ ] **TypedDict structural assignability** (~17): E0014 readonly_consistency (5)/readonly_inheritance (2)/inheritance (1); E0093 extra_items/ReadOnly/final/update (7+); PEP 728/705.
- [ ] **`assert_type` flow narrowing** (E0053 ~11): isinstance/`is`/TypeGuard/TypeIs (exceptions_context_managers 4, literals_interactions 4, narrowing_typeguard 2, narrowing_typeis 1) + Union/tuple-unpack syntactic equivalence (annotations_typeexpr 1, tuples_unpacked 3).
- [ ] **Constructors** (E0111 9): dataclass_transform class/converter/meta, NamedTuple subscript/inheritance, type-erasure.
- [ ] **Recursive generic aliases** (4): TypeVar substitution (GenericTypeAlias1/2 in aliases_recursive).
- [ ] **E0012 TypeVarTuple arity** (4): needs argument-type resolution in E0012.
- [ ] **E0130 TypeVar scoping** (3), **E0047 ParamSpec annotations** (≈4), and scattered singles (E0048/E0054/E0069/E0060/E0115/E0094/E0078/E0092/E0139/E0112/E0140/E0041/E0143).
- [ ] **`if not TYPE_CHECKING:` block bodies** (E0014 directives_type_checking 1): resolver must mark statements unreachable to the checker.

---

## SHOWSTOPPER: BSK-E0149 treats docstring text as `class` / `def` definitions

**Reported**: 2026-05-23 — found in the wild on `StoryTowns/scripts/provision_nimblesite_agent.py`.
**Severity**: SHOWSTOPPER. Hard errors on perfectly valid Python text in module docstrings. Any docstring containing a bracketed token after a line that happens to begin with the word `class` (e.g. our own `[SPEC-ID]` cross-references — see CLAUDE.md "ALL CODE **MUST** REFER TO A SPEC-ID") will misfire.

**Files**:
- `crates/basilisk-checker/src/rules/e0149/violations.rs:19-38` (`collect_pep695_type_params`)
- `crates/basilisk-checker/src/rules/e0149/mod.rs:78-117` (the line-iteration driver)

**Root cause**: `collect_pep695_type_params` (and the rest of the rule) scans `source.lines()` and treats any line whose trimmed prefix is `class `, `def `, `async def `, or `type ` as a real definition. It has **no awareness of string/docstring/comment boundaries**. The driver loop in `mod.rs:78` shares the same blind spot.

**Minimal repro**:
```python
"""Docstring.

class as the public Supabase anon key — see [AI-API-AUTH] in
foo bar [AI-API-AUTH].
"""
```
The docstring line `class as the public ... [AI-API-AUTH] ...` is parsed as `class as[the public ... AI-API-AUTH ...]:` → `AI-API-AUTH` registered as a PEP 695 type param. Then `foo bar [AI-API-AUTH].` (still inside the docstring) is flagged as module-level use of an out-of-scope type param.

**Why this is bad**:
1. Direct violation of CLAUDE.md: "Regex = ⛔️ ILLEGAL. Use the proper parsing mechanism - usually ruff". This rule is doing line-prefix string matching, which is the moral equivalent.
2. Hard errors on docstring prose erode the entire product's credibility — the user's exact reaction was "WTF is this? a bug?"
3. Our own spec-ID convention (`[GROUP-TOPIC]` references in docstrings) is the most likely trigger.

**Fix**:
- Stop iterating raw source lines. Drive the rule off `ResolvedModule.classes` / `functions` / `type_statements` (already available — see `mod.rs:172-185` for the right pattern, used by `check_type_alias_misuse`).
- For the module-level type-param-use check, walk AST statements, not text lines.
- The line-based helpers (`collect_pep695_type_params`, `check_module_level_type_param_use`, `check_pep695_bound_cross_references`, etc.) should consume parsed nodes, not `&str` lines.

**Related**: the older E0149 entry in the "Completed" list above fixed a *different* line-scanning bug (RHS subscripts of `type` statements). That fix did not address docstring/string-content misclassification. Both come from the same underlying anti-pattern — the whole rule needs to be re-grounded on the AST.

**Verification**: add a conformance fixture with a module docstring containing `class ...` / `def ...` prefixes and `[Name]` bracketed tokens, and assert zero E0149 diagnostics. Also add a fixture with `[SPEC-ID]` cross-references (matching our own convention).
