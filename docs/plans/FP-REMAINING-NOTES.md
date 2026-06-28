# Remaining Conformance FP Notes (e0014/e0093 cluster) {#FPNOTES}

Per-FP investigation notes for the e0014/e0093 false-positive cluster. Conformance
is measured ONLY by the unmodified `python/typing` scorer with every rule enabled
(see [CHKARCH-CONFORMANCE]); a FP = an Error on a line with no `# E`. Any fix here
must add ZERO new `missed` and flip NO file PASS→FAIL. Verify with
`cargo test --test conformance_tests -- --nocapture` + grep `  FP ` / `  DEBUG`.

## Landed (do not re-investigate) {#FPNOTES-LANDED}

These three items are implemented; kept as a pointer so they are not re-opened:

- **literals_semantics.py `L[` Literal-alias parsing** — `parse_complex_annotation`
  (`crates/basilisk-checker/src/types_parsing.rs`) now accepts the `l[` prefix
  alongside `literal[`.
- **typeddicts_readonly_consistency.py TypedDict-to-TypedDict assignability** — E0014
  runs a PEP-705-aware structural check
  (`crates/basilisk-checker/src/rules/assignment_compatibility/typeddict_struct.rs`,
  via `strip_typeddict_qualifiers`).
- **aliases_recursive.py recursive generic aliases** — the value-alias matcher
  (`crates/basilisk-checker/src/rules/assignment_compatibility/alias_match.rs`,
  `collect_generic_aliases` + `resolve_generic`) handles container-bodied generic
  aliases with TypeVar substitution.

---

## OPEN — typeddicts_readonly_update.py:34 — remove `"update"` from the blanket ban {#FPNOTES-TYPEDDICTS-READONLY-UPDATE}

Line 34: `a.update(b)  # OK` — `a: A`(`x: ReadOnly[int]`,`y:int`), `b: B`(`x: NotRequired[Never]`,`y: ReadOnly[int]`). Valid per PEP 705 (source `x` is `Never`).

**Root cause:** `crates/basilisk-resolver/src/visitor/core.rs` (`const DISALLOWED: &[&str] = &["clear","pop","popitem","setdefault","update"]`) blanket-bans these methods for non-`extra_items` TypedDicts (`DisallowedMethodCall`). `.update()` is ReadOnly/`Never`-unaware → fires on L34. (Note: a `disallowed_mutator_flagged()` helper exists but the method name is still in the blanket set, so it fires before the specialized logic can run.)

**Fix:** remove `"update"` from that `DISALLOWED` list (keep clear/pop/popitem/setdefault).

**TP-safety:** L23 `a1.update(a2)  # E` stays caught by **E0056** (ReadOnly-aware `UpdateCall` in `final_readonly.rs`) — confirmed firing on L23 independently. Only suite `.update()` uses are L23/L34, so no regression. (`.clear()` TPs in typeddicts_operations.py:47,62 untouched.)
