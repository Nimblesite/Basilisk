# Conformance Integrity Audit {#CHKARCH-CONFORMANCE-INTEGRITY-AUDIT}

**Subject:** Basilisk's alias-validation rules were fitted to the contents of the conformance test files rather than to the typing specification.

**Audited tree:** `bidirectionaltype-inference` @ `c041759`. Comparison baseline `main` @ `da74283`.
**Conformance state at audit time:** 141 / 141 files `PASS`.
**Audit performed:** 2026-08-05.

---

## 0. Statement

We found that at least one Basilisk rule earns its conformance result by pattern-matching the text of the test file it is scored against, not by implementing the rule the file tests. We are publishing the finding, the method used to detect it, the full audit of the rest of the checker, and the current state of the fix — including the parts that are still broken.

We did not find this ourselves. It was reported from outside, in [issue #379](https://github.com/Nimblesite/Basilisk/issues/379), from a [public reproduction](https://x.com/cyanchanges/status/2083115048143364512). That is itself a finding, and it is covered in §6.

Our conformance number is self-measured. Where a passing file is carried by predicates shaped to that file, the honest statement is that **the file passes and the rule is not implemented**. That is the case for the files listed in §3.

---

## 1. The primary defect

`is_invalid_rhs` decides whether the right-hand side of a type alias is a valid type expression by running prefix and substring tests against the **raw source text** of the RHS. On `main` @ `da74283` it is duplicated verbatim in two rules:

- `crates/basilisk-checker/src/rules/aliases_type_statement.rs:44` — rewritten at `c041759` (§5.1)
- `crates/basilisk-checker/src/rules/aliases_implicit.rs:97` — **unchanged at `c041759`**

The analysis below is of the function as shipped on `main`, because that is what the published conformance result was produced from. §5 gives the current state of each copy.

```rust
fn is_invalid_rhs(rhs: &str) -> bool {
    let rhs = rhs.trim();
    if rhs == "True" || rhs == "False" { return true; }
    if rhs.chars().next().is_some_and(|c| c.is_ascii_digit()) { return true; }
    if rhs.starts_with("f\"") || rhs.starts_with("f'") { return true; }
    if rhs.starts_with('[') { return true; }
    if rhs.starts_with('{') { return true; }
    if rhs.starts_with('(') && paren_has_top_level_comma(rhs) { return true; }
    if has_top_level_token(rhs, " if ") { return true; }
    if has_top_level_token(rhs, " or ") || has_top_level_token(rhs, " and ") { return true; }
    if rhs.contains("lambda") { return true; }
    if rhs.starts_with("eval(") { return true; }
    false
}
```

### 1.1 Every branch maps to exactly one test line

Against [`conformance/tests/aliases_type_statement.py`](https://github.com/python/typing/blob/main/conformance/tests/aliases_type_statement.py#L37-L49):

| Conformance test line | Branch that catches it |
| --- | --- |
| `eval("".join(map(chr, [105, 110, 116])))` | `starts_with("eval(")` |
| `[int, str]` | `starts_with('[')` |
| `((int, str),)` | `starts_with('(') && paren_has_top_level_comma` |
| `[int for i in range(1)]` | `starts_with('[')` |
| `{"a": "b"}` | `starts_with('{')` |
| `(lambda: int)()` | `contains("lambda")` |
| `[int][0]` | `starts_with('[')` |
| `int if 1 < 3 else str` | `has_top_level_token(" if ")` |
| `var1` | `is_non_type_name` |
| `True` | `== "True" \|\| == "False"` |
| `1` | `is_ascii_digit()` |
| `list or set` | `has_top_level_token(" or ")` |
| `f"{'int'}"` | `starts_with("f\"")` |

**Coverage of the test file: 13 / 13.**
**Content not required by the test file: 3 items** — `False`, `" and "`, and the negative-number branch. Each is the trivial symmetric twin of a branch that *was* required.

There is no branch for any other call, for `+`/`-`/`*`, for comparisons, for `not`, for unary operators, for bytes literals, for starred or walrus expressions, or for attribute access on a subscript.

### 1.2 The decisive detail

`starts_with("eval(")` hardcodes one builtin function name as a **source-text prefix**. `eval` has no standing in PEP 613, PEP 695, or the typing spec. The only reason to name it is that `BadTypeAlias1` in the conformance suite is spelled `eval(...)`. `int("3")` is the identical spec violation and is accepted.

It appears in three separate files on `main`, of which two remain at `c041759`:

| File | `main` @ `da74283` | `c041759` |
| --- | --- | --- |
| `rules/aliases_type_statement.rs` | `:82` | removed — rule rewritten (§5.1) |
| `rules/aliases_implicit.rs` | `:157` | `:157` — **still present** |
| `rules/annotations_forward_refs/type_checks.rs` | `:113` | `:113` — **still present** |

### 1.3 The same shape elsewhere in the same rule

Not confined to `is_invalid_rhs`. In `aliases_implicit.rs`, fitted to [`aliases_implicit.py:76-81`](https://github.com/python/typing/blob/main/conformance/tests/aliases_implicit.py#L76-L81):

| Location | What it does | Fitted to |
| --- | --- | --- |
| `:693` | Emits a hard error on the guess `all_simple && args.len() > 1`; its own comment says the ParamSpec arg is "**probably** wrong" | `GoodTypeAlias9[int, int]` |
| `:757` | `is_assignable_to_bound` implements `int`/`float`/`complex` and returns **accept** for every other bound | `TFloat = TypeVar("TFloat", bound=float)` — the suite's only bounded TypeVar |
| `:407` | Treats a module variable as an implicit type alias only if its name **starts with an uppercase ASCII letter** | The suite names them `GoodTypeAlias*` / `ListAlias` |
| `:76` | Recovers `TypeAlias as X` imports by `match_indices` over raw import text | — |

---

## 2. Reproduce it yourself

```bash
git clone https://github.com/Nimblesite/Basilisk && cd Basilisk
cargo build --release --bin basilisk

# TypeVar bound checking exists only for the numeric tower
cat > bound.py <<'EOF'
from typing import TypeVar
TStr   = TypeVar("TStr",   bound=str)
TFloat = TypeVar("TFloat", bound=float)
AliasStr   = list[TStr]
AliasFloat = list[TFloat]
def f(a: AliasStr[int])   -> None: ...   # should error — silent
def g(b: AliasFloat[str]) -> None: ...   # errors (matches the suite)
EOF
./target/release/basilisk check bound.py

# Alias detection depends on the first letter being uppercase
cat > case.py <<'EOF'
list_or_set = list | set
ListOrSet   = list | set
x = list_or_set()   # should error — silent
y = ListOrSet()     # errors (matches the suite)
EOF
./target/release/basilisk check case.py

# Import alias recovery depends on exactly one space around `as`
cat > spaces.py <<'EOF'
from typing import TypeAlias as  TA
from typing import TypeAlias as TB
X: TA = [int, str]   # should error — silent
Y: TB = [int, str]   # errors
EOF
./target/release/basilisk check spaces.py
```

Verified output at `c041759`: exactly one diagnostic per file — the control case in each pair. Each first case is a false negative.

---

## 3. Effect on our conformance result

Four conformance files list an alias rule among the rules that carry them:

| File | Basilisk rules credited | Status |
| --- | --- | --- |
| `aliases_explicit.py` | `aliases_implicit` | PASS |
| `aliases_implicit.py` | `aliases_implicit`, `annotations_forward_refs`, `generics_defaults_specialization` | PASS |
| `aliases_type_statement.py` | `aliases_type_statement`, `generics_syntax_scoping` | PASS |
| `annotations_typeexpr.py` | `aliases_implicit`, `annotations_forward_refs`, `annotations_typeexpr` | PASS |

These files pass. The errors on their `# E` lines are reported. **We are not claiming the underlying rules are implemented to spec**, and for `aliases_implicit` they demonstrably are not.

We are not publishing a revised percentage. A number produced by the same suite that the code was fitted to would not measure the thing in question. §5 describes what we are doing instead.

---

## 4. Audit of the rest of the checker

### 4.1 Method

Executed over `crates/basilisk-checker/src` at `c041759`, excluding test files. Five signatures, chosen because each is a way a checker can look correct on a fixture without implementing a rule:

1. Hardcoded identifiers drawn from conformance fixtures.
2. Branching on the file name or path under test.
3. Hardcoded single-symbol string literals used as behavioural triggers (the `eval(` shape).
4. Reconstruction of Python structure from source text rather than the AST.
5. Accept-all fallback arms and disclosed guesses.

### 4.2 What we looked for and did **not** find

These matter as much as the positives, and each was checked directly:

- **No hardcoded conformance identifiers in executable code.** No `BadTypeAlias*`, `GoodTypeAlias*`, or `var1` string literals outside doc comments.
- **No branching on file name or path.** `module.path.contains / ends_with / starts_with` occurs **0 times** across all rules. Every `.py` string literal in the crate is a synthetic path in a test fixture or a parser call.
- **No conformance-result tampering.** `conformance_status.csv` is generated; no rule is disabled or unregistered.

The defect is narrower than "the checker is faked". It is specific and it is real.

### 4.3 Category A — conformance-fitted predicates (confirmed)

The `eval(` prefix in three files (§1.2), and the four `aliases_implicit` heuristics in §1.3. These are the confirmed instances. Each is now a tracked issue (§7).

### 4.4 Category B — source-text scanning (structural, pre-existing, tracked)

Reconstructing Python structure from text is the technique that *permits* Category A. It is widespread and was already under demolition before this audit:

| Measure | Count (non-test) |
| --- | --- |
| Rule files in the crate | 244 |
| Files slicing expression text out of source (`slice_span`) | 81 |
| Files scanning by line (`.lines()`) | 18 |
| Statement reconstruction by keyword prefix (`starts_with("class "`, `"def "`, …) | 32 sites |
| Text predicates in rule bodies (`starts_with` / `ends_with` / `contains` on strings) | 246 sites |

Underneath these sits `types_parsing.rs` — 417 LOC that build the internal type representation by **parsing annotation text**, including a `to_ascii_lowercase()` on the annotation, which collides a user-defined class `Int` with the builtin `int`, and which maps `object` to `Any`. It carries this header already:

> ⚠️ LEGACY — condemned under [TYPEINF-LEGACY]. … No new code may call into this module.

It still has **16 non-test call sites across 9 files**, including the LSP hover path. Condemned is not the same as gone, and we should not have described it as if it were.

For proportion: the real engine (`bidir/` 2,965 LOC, `narrow/` 2,253 LOC, `subtyping.rs` 286 LOC) is roughly ten times the size of the legacy remnants, and most rules run against it. The migration is real and most of the way done. It is not finished, and the unfinished part is where this defect lived.

### 4.5 Category C — disclosed conservatism

13 accept-all `_ => true` fallback arms, and 53 comment lines disclosing an approximation. By keyword — these overlap, so they sum to more than 53: "conservative" 26, "heuristic" 8, "assume" 6, "approximate" 5, "simplified" 5, "for now" 3, "best-effort" 2, "probably" 1.

Most of these are honest, documented deferrals — e.g. `assignment_compatibility/alias_match.rs:142` explicitly records that textual substitution is unsound for a `ParamSpec`-parameterised `Callable` and routes those forms to the path that models them properly. We are not calling those defects.

The exception is `is_assignable_to_bound`, where `_ => true` is not a documented deferral but the entire remainder of the type system. A fallback that accepts everything outside a three-element set is indistinguishable from an unimplemented check, and it should never have been described as "conservative".

---

## 5. Remediation status — measured, not asserted

> **Note on §5.1.** It records a rewrite made before the current policy. Rewriting is no
> longer the response to text-matched logic — deletion is (§7). §5.1 is kept because its
> measurement is the clearest evidence for *why*: a rewrite that fixes the headline case
> and leaves the rule broken still reads as a fix.

### 5.1 `aliases_type_statement` — rewritten, **partially** effective

The rule now validates the `StmtTypeAlias` value node structurally on the Ruff AST. All 13 conformance cases are rejected for the right structural reasons, and it catches forms the text scanner never could.

**It does not fully close #379.** Measured at `c041759` against that issue's own minimal cases:

| Case | Expected | Actual |
| --- | --- | --- |
| `type c = "the" + list["of genshin"].impact…` | error | **error** ✅ |
| `type A = "the" + "thing"` | error | **error** ✅ |
| `type E = 1 + 2` | error | **error** ✅ |
| `type B = list["of genshin"]` | error | **silent** ❌ |
| `type D = list[int].attr` | error | **silent** ❌ |

`Expr::Attribute(_)` returns `true` unconditionally, so attribute access on a subscript passes. Subscript *arguments* are deliberately not descended into, so an unparseable forward-reference string is never validated. Two of the four cases in the report that triggered this audit are still open. #379 stays open and we are not claiming it fixed.

The headline repro now errors, but only on its `+` operator. Deleting the leading `"the" + ` leaves the rest of that line accepted in full:

```python
type c = list["of genshin"].impact.updates.that.you.should.definitely["try"]. \
  because.this["is not"].a.real.type.checker.wtf.ls["this"]
```
```
All checked. No issues found.
```

A headline case passing is not evidence that the rule behind it works. That is the same inference error that produced this audit, and we are flagging it against ourselves here deliberately.

### 5.2 `aliases_implicit` — not started

Unchanged from `main` except for routing `is_assignable_to_bound` through a `SubtypingContext`; the `_ => true` arm is intact. All four defects in §1.3 reproduce at `c041759`, verified with A/B controls:

| Defect | Control (matches suite) | Failing case |
| --- | --- | --- |
| Bound checking | `bound=float` → errors | `bound=str` → silent |
| Alias detection | `ListOrSet()` → errors | `list_or_set()` → silent |
| Import aliases | `TypeAlias as TB` → errors | `TypeAlias as  TA` → silent |
| ParamSpec position | `Alias9[int, int]` → errors | `Alias9[int, [str]]` → silent |
| Nesting | `NotGeneric[int]` → errors | `list[NotGeneric[int]]` → silent |

---

## 6. Process failures

1. **The conformance suite cannot detect this class of defect, by construction.** It is the artefact the code was fitted to. A suite cannot audit code written against it. Every green run reinforced the wrong conclusion.
2. **An outside reporter found it, not us.** Nothing in our review or CI asks "would this rule work on input the suite does not contain?"
3. **A ratchet on pass-percentage rewards this failure mode.** When the only metric that can move is a number the code can be fitted to, fitting is the lowest-cost way to move it. That is a design flaw in our incentives, not a lapse by any individual.
4. **"Condemned" was doing work that "deleted" should have done.** `types_parsing.rs` was labelled legacy and still had 16 live call sites.

---

## 7. What we are changing

The correction is **audit and deletion**, not repair. Code that decides from the
spelling of its input rather than its meaning is removed, and what survives is code
that analyses Python. Nothing here is rewritten in place: rewriting preserves the claim
that the rule worked, and that claim is what has to go.

- **Text-matched logic is deleted, not fixed.** On finding it: write a test that fails
  because of it, delete the code, and report what went and why — no fix, no rewrite, no
  TODO ([CHKARCH-TEXT-MATCHED-LOGIC](specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TEXT-MATCHED-LOGIC)).
  **A failing test that pins real incorrect behaviour is worth more than a passing
  fixture carried by logic that does not analyse code.** What gets built back is a
  deliberate, separate decision.
- **A smaller checker is the expected outcome, and an acceptable one.** Rule count,
  diagnostic coverage, and the conformance number are all expected to fall. Each drop is
  reported. None is reverted, and none is a reason to keep code that was never doing the
  work.
- **The pass-percentage floor is the mechanism that caused this** (§6.3) and is no
  longer a target ([CHKARCH-CONFORMANCE](specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFORMANCE)).
  No conformance figure is published or quoted, and there is no re-submission to
  `python/typing` until the semantics-preserving mutation harness passes clean and an
  external audit has run.
- **Build the control that would have caught this.** Semantics-preserving mutation —
  aliased imports, reformatting, reordering, consistent renaming → identical diagnostics
  ([CHKARCH-TESTING-SEMANTIC-MUTATION](specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING-SEMANTIC-MUTATION)).
  It does not exist yet, which is why every green run reinforced the wrong conclusion.
  Until it does, no rule is verified.
- **Off-suite tests are mandatory** for every surviving rule, derived from the spec
  grammar and real code, explicitly **not** from `conformance/tests/`.
- **Ban hardcoded symbol names as behavioural triggers.** A rule may not key on a
  specific identifier spelling unless the spec names that symbol.
- **`_ => true` is an unimplemented check.** An accept-all arm either states which cases
  it defers and to which path, or it is deleted along with the rule that relies on it.
- **Finish the AST work.** Category B is the enabling condition; the tracked plans are
  [`CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md`](plans/CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md)
  and [`CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md`](plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md).
  `aliases_implicit.rs` was in neither inventory; it was added as part of this audit.

## 8. Issue index

| Issue | Subject | State |
| --- | --- | --- |
| [#379](https://github.com/Nimblesite/Basilisk/issues/379) | Original external report — substring matching on type-statement RHS | Open; **partially** fixed (§5.1) |
| [#408](https://github.com/Nimblesite/Basilisk/issues/408) | Integrity umbrella — the 1:1 mapping and full scope | Open |
| [#409](https://github.com/Nimblesite/Basilisk/issues/409) | ParamSpec argument check is a shape guess; never identifies the ParamSpec position | Open |
| [#410](https://github.com/Nimblesite/Basilisk/issues/410) | `is_assignable_to_bound` accepts every bound outside `int`/`float`/`complex` | Open |
| [#411](https://github.com/Nimblesite/Basilisk/issues/411) | Implicit aliases detected by uppercase-first-letter naming heuristic | Open |
| [#412](https://github.com/Nimblesite/Basilisk/issues/412) | `TypeAlias as X` resolved by substring scan, duplicating the real name cascade | Open |

---

*All measurements in this document are reproducible from `bidirectionaltype-inference` @ `c041759` using the commands in §2. Counts in §4 were produced by grep over `crates/basilisk-checker/src` excluding test files; the exact queries are recorded in §4.1.*
