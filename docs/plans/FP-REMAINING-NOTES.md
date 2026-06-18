# Remaining Conformance FP Notes (for B2's e0014/e0093 lane)

Compiled by Basilisk3 from deep per-FP investigation. After B3's NamedTuple
(E0111/E0143) + deprecated (E0115) fixes landed, the suite is at **11 FPs**, all
in the E0014/E0093 cluster below. Each entry: root cause, minimal TP-safe fix,
and the critical "do not drop a `# E`" constraint.

> Harness fact: a FP = an Error on a line with no `# E`. Fix must add ZERO new
> `missed` and flip NO file PASS→FAIL. Verify with
> `cargo test --test conformance_tests -- --nocapture` + grep `  FP ` / `  DEBUG`.

---

## 1. literals_semantics.py:39 — BSK-E0014 (EASIEST, 1 FP)

Line 39: `x1: Literal[3, b"bar", True, "foo", None] = a` inside
`def func4(a: L[None, 3] | L[3, "foo", b"bar", True])` where `from typing import Literal as L`.

**Root cause:** `parse_complex_annotation` (`crates/basilisk-checker/src/types_parsing.rs`,
the `literal[...]` branch) only recognizes the lowercased spelling `literal[`. The
`L[...]` alias falls through to `InferredType::Named("l[none, 3]")`, so the param
type mis-parses and `is_assignable_to` rejects the (reordered-but-equal) Literal union.

**Fix:** in that branch, also accept the `l[` prefix as a `Literal` alias (mirror
E0129's `contains_literal_subscript` precedent which already handles `L[`).

**TP-safety:** the 4 `# E` lines (10,24,25,33) all spell `Literal[...]` (not `L[`),
unaffected. `literals_semantics.py` is the only suite file using the `L` alias;
none of its `L[...]` uses are `# E` for E0014. Line 33's `# E` is owned by E0129.

---

## 2. typeddicts_readonly_update.py:34 — BSK-E0093 (ONE-LINER, 1 FP)

Line 34: `a.update(b)  # OK` — `a: A`(`x: ReadOnly[int]`,`y:int`), `b: B`(`x: NotRequired[Never]`,`y: ReadOnly[int]`). Valid per PEP 705 (source `x` is `Never`).

**Root cause:** `crates/basilisk-resolver/src/visitor/core.rs:~417` has a blanket
`DISALLOWED = ["clear","pop","popitem","setdefault","update"]` ban for non-extra_items
TypedDicts (`DisallowedMethodCall`). `.update()` is ReadOnly/`Never`-unaware → fires on L34.

**Fix:** remove `"update"` from that `DISALLOWED` list (keep clear/pop/popitem/setdefault).

**TP-safety:** L23 `a1.update(a2)  # E` stays caught by **E0056** (ReadOnly-aware
`UpdateCall` in `final_readonly.rs`). Confirmed: E0056 fires on L23 independently.
Only suite `.update()` uses are L23/L34, so no regression. (`.clear()` TPs in
typeddicts_operations.py:47,62 untouched.)

---

## 3. typeddicts_readonly_consistency.py:34,35,41,78,79 — BSK-E0014 (CAREFUL, 5 FPs)

Var-to-var TypedDict assignments (e.g. L34 `v1: A1 = b` where `b: B1`). All valid by
PEP 705 width / read-only subtyping.

**Root cause:** E0014 (`e0014/mod.rs` `check_local_vars`→`check_vars`→`is_assignable_to`
`types.rs:~339`) compares two `Named` TypedDicts by **base-name equality only**. It
fires on ALL 12 var-to-var assignments here — both the 5 `# OK` and the 7 `# E`.

**⚠ DO NOT blanket-skip TypedDict-to-TypedDict locals.** That drops all 12 firings,
and E0093 will NOT recover the 7 TPs because `check_typeddict_assignability`
(`e0093/type_consistency.rs:~39`) iterates only `module.module_vars` — it never
traverses function bodies. Result would be +7 missed → conformance regression.

**Fix (surgical):** in E0014, when BOTH declared and inferred are TypedDict `Named`
types, run a PEP-705-aware structural assignability check (reuse
`basilisk_resolver::strip_typeddict_qualifiers`, which strips ReadOnly/Required/
NotRequired/Annotated) applying width + read-only-width-subtyping. Keeps the 7 TPs
(now firing for the right reason) and clears the 5 FPs.

**TP-safety:** `# E` lines here = 37,38,40,81,82,84,85. The only other E0014 TypedDict
catch in the suite is extra_items.py:352 (RHS not a TD var) — unaffected.

---

## 4. aliases_recursive.py:61,62,67,68 — BSK-E0014 (HARDEST, 4 FPs)

Recursive **generic** aliases:
`GenericTypeAlias1 = list["GenericTypeAlias1[T1]" | T1]` etc., used as
`g1: SpecializedTypeAlias1 = [...]`, `g2: GenericTypeAlias1[str] = [...]`.

**Root cause:** the recursive value-alias matcher (`e0014/alias_match.rs`,
`collect_union_aliases`) is gated to `Union`-bodied aliases only and explicitly
excludes `list[...]`-bodied generic aliases needing TypeVar substitution. So these
parse to `InferredType::List(...)`, fall to `is_assignable_to`, hit `List`-vs-`Named`
→ `_ => false` → FP.

**Fix:** extend the matcher to container-bodied generic aliases with a TypeVar
binding env: bind subscript args (`GenericTypeAlias1[str]` → T1=str), resolve bare
TypeVar leaves through the env, and re-expand the recursive self-reference with the
same env. Also relax the `!name.contains('[')` guard in `mod.rs` so subscripted
alias annotations enter the matcher.

**⚠ TP-safety:** lines 63 and 69 are `# E` and use the SAME annotations as the FP
lines — they differ only by a `float` leaf where the alias permits `str`/`str|int`.
A blanket skip of generic-alias annotations would drop those 2 TPs. Must be a
structural matcher (positive-match semantics already reject `float`→`str`).

---

### Status
- B3 lane (E0111/E0143/E0115) = DONE, verified: 144/146, caught=917, missed=37
  (unchanged, both pre-failing files), suite FP 21→11.
- Items 1 & 2 above are low-risk quick wins; 3 & 4 need structural work but the
  TP-safety traps are spelled out.
