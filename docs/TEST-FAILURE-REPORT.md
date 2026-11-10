# Test Failure Report — branch `fixes`

**Date:** 2026-08-08
**Author:** secondary agent (checker lane)
**Purpose:** hand-off inventory for the main agent.

---

## TL;DR

| | |
|---|---|
| Tests **run** | 1,192 |
| Tests **passing** | 1,031 |
| Tests **failing** | **161** (all in `basilisk-resolver`) |
| Tests **unrunnable** (compile-blocked) | **~5,700** |
| Compile errors blocking `basilisk-checker` | **35** |

Two independent problems:

1. **`basilisk-resolver` — 161 failing tests.** These are the *expected, correct* residue of the text-match purge. Collectors were gutted to stubs; their tests remain and now fail. Per CLAUDE.md this is the desired outcome, not a regression to undo.
2. **`basilisk-checker` — 35 compile errors.** The same purge orphaned ~15 call sites. Until these resolve, **4,300 checker tests cannot run at all**, including 52 golden tests that have *never* executed.

---

## 1. What actually ran

### Passing (fully green)

| Crate | Passed | Failed |
|---|---:|---:|
| `basilisk-stubs` | 278 | 0 |
| `basilisk-uv` | 137 | 0 |
| `basilisk-config` | 72 | 0 |
| `basilisk-parser` | 29 | 0 |
| `basilisk-common` | 15 | 0 |
| `basilisk-db` | 13 | 0 |
| `basilisk-canonical` | 8 | 0 |
| `basilisk-profiler-protocol` | 7 | 0 |
| `basilisk-buildinfo` | 0 | 0 |
| **Subtotal** | **559** | **0** |

### Failing

| Test target | Passed | **Failed** |
|---|---|---|
| `basilisk-resolver` lib | 10 | 0 |
| `tests/annotation_tests.rs` | 4 | **14** |
| `tests/class_tests.rs` | 37 | **8** |
| `tests/function_tests.rs` | 35 | **1** |
| `tests/misc_tests.rs` | 13 | **11** |
| `tests/mutant_tests.rs` | 58 | **31** |
| `tests/protocol_tests.rs` | 35 | **25** |
| `tests/resolution_tests.rs` | 81 | **1** |
| `tests/type_system_tests.rs` | 131 | **36** |
| `tests/typeddict_tests.rs` | 68 | **34** |
| **Subtotal** | **472** | **161** |

> **Method note 1.** `--no-fail-fast` was used **for diagnosis only**, to enumerate every failing target rather than stopping at the first. The `make test` gate itself remains fail-fast, per CLAUDE.md. No gate, threshold, or config was touched.
>
> **Method note 2.** Per-crate counts above come from running each crate individually. A batched `cargo test -p A -p B …` interleaves output across targets and under-reports; the batched figures were discarded.

---

## 2. The 161 resolver failures — root cause

**These are purge residue, and they are correct.** The dominant pattern — **79 of the 161** — is an assertion that a collection is non-empty, failing because the collector behind it was reduced to a stub. The remaining 82 are `assert_eq!`/field assertions with the same underlying cause.

Proof — [`crates/basilisk-resolver/src/visitor/calls_and_reveal.rs:23`](crates/basilisk-resolver/src/visitor/calls_and_reveal.rs#L23):

```rust
pub(super) fn collect_reveal_type_calls(_stmts: &[Stmt]) -> Vec<RevealTypeCallInfo> {
    Vec::new()
}
```

The parameter is underscored; the body returns empty. Every `!resolved.reveal_type_calls.is_empty()` assertion fails by construction.

### Failures grouped by the collection that is now empty

Complete breakdown of the 79 non-empty assertions:

| Empty collection | Failing tests | Capability lost |
|---|---:|---|
| `protocol_instantiation_violations` | 18 | Protocol/ABC instantiation detection |
| `assert_type_calls` | 13 | `assert_type()` collection |
| `protocol_runtime_checkable_violations` | 10 | `@runtime_checkable` enforcement |
| `enum_value_type_violations` | 7 | Enum member value typing |
| `reveal_type_calls` | 6 | `reveal_type()` collection |
| `namedtuple_defs` | 5 | Functional `NamedTuple` forms |
| `readonly_violations` | 4 | `ReadOnly` qualifier enforcement |
| `type_alias_defs` | 3 | PEP 613 / `TypeAliasType` |
| `annotated_direct_call_spans` | 3 | `Annotated[...]` call sites |
| `typeddict_key_violations` | 2 | TypedDict key/value checks |
| `literal_string_enum_mismatches` | 2 | `Literal` vs enum mismatch |
| `generic_non_typevar_args` | 2 | Non-TypeVar args in generic brackets |
| `typeddict_calls` | 1 | Functional `TypedDict` form |
| `newtype_calls` | 1 | `NewType` collection |
| `multiple_unbounded_tuple_spans` | 1 | Unbounded-tuple arity |
| `abstract_violations` | 1 | Abstract instantiation |
| **Total** | **79** | |

The other 82 failures are `assert_eq!` and field-level assertions (`is_dataclass`, `is_staticmethod`, `field_names.len()`, `lhs_name == …`) failing for the same reason — the data was never collected.

### These are the *good* failing tests

Several failures are exactly the semantic-mutation tests CLAUDE.md asks for. They are named for it:

- `test_decorators::builtin_decorator_flags_do_not_depend_on_spelling`
- `test_base_subscript_03::direct_tuple_index_check_does_not_depend_on_annotation_spelling`
- `test_base_subscript_03::key_lambda_tuple_index_check_does_not_depend_on_annotation_spelling`
- `test_typeddict_keys_01::typeddict_value_type_check_does_not_depend_on_builtin_spelling`

**Do not delete or weaken these.** They are the accurate map of what Basilisk cannot yet do. Restoring the deleted spelling-matchers to make them pass would recreate the exact defect that got Basilisk pulled from the conformance results.

---

## 3. Silently-false flags — active landmine

[`crates/basilisk-resolver/src/visitor/function_info.rs:129-130`](crates/basilisk-resolver/src/visitor/function_info.rs#L129-L130):

```rust
is_overload:     has_decorator_form(bindings, &func.decorator_list, TypingForm::Overload),  // ✅ real
is_staticmethod: false,   // ❌ HARDCODED
is_classmethod:  false,   // ❌ HARDCODED
is_no_type_check: has_decorator_form(bindings, &func.decorator_list, TypingForm::NoTypeCheck), // ✅ real
```

`is_staticmethod` / `is_classmethod` **always evaluate false**. Confirmed live by the failing test `test_decorators::builtin_decorator_flags_do_not_depend_on_spelling`, and by `bare_method.is_some_and(|method| method.is_staticmethod)`.

**Consequence:** any rule that swaps `decorator_spelled("staticmethod")` → `func.is_staticmethod` will **compile cleanly and silently disable itself**. I deliberately left these three call sites broken rather than "fix" them into silent no-ops:

- [`calls_argument_count/method_binding.rs:135`](crates/basilisk-checker/src/rules/calls_argument_count/method_binding.rs#L135)
- [`missing_parameter_annotation.rs:191`](crates/basilisk-checker/src/rules/missing_parameter_annotation.rs#L191)
- [`missing_parameter_annotation.rs:195`](crates/basilisk-checker/src/rules/missing_parameter_annotation.rs#L195)

The resolver-lane agent is implementing the real flags plus `is_abstractmethod`. **Swap all three only after that lands.**

Flags I verified genuinely resolve through `bindings` and are safe to use today: `is_overload`, `is_no_type_check`, `is_class_var`, `is_enum`, `is_protocol`, `is_namedtuple`.

---

## 4. The 35 checker compile errors

Blocking **4,300** `basilisk-checker` tests, plus **870** LSP and **532** CLI tests downstream (~5,700 total).

> These three figures are counts of `#[test]` attributes in each crate's sources, not measured run counts — the crates do not compile, so no runner has ever enumerated them. Treat as the order of magnitude at risk, not an exact total.

Grouped by root cause — count, and what each needs:

### A. `AnnotationResolver::is_structural_target` deleted — 8 errors, 5 files
`assignment_compatibility/mod.rs:204,393` · `narrowing_typeis_2.rs:255` (×2) · `returns_compatibility.rs:112` · `returns_compatibility_2.rs:89` · `shared/judge.rs:105,114`

Needs the `Tables.structural` set and `class_is_structural()` rebuilt on resolved identity. **Design decision required.**

### B. TypedDict qualifier model gutted — 13 errors, 4 files
`typeddicts_extra_items/checks.rs:13,90,140,160,182,226,271,297` · `typeddicts_extra_items/model.rs:15,167,175` · `assignment_compatibility/typeddict_struct.rs:13` · `typeddicts_operations/type_consistency.rs:21`

`strip_typeddict_qualifiers`, `strip_qualifiers`, `Qualifier`, `ExtraItems.readonly`, `ExtraItems.qualifier` all deleted as text-based strippers.

**Do not re-add them to the resolver** (resolver lane's explicit instruction). Rebuild qualifier peeling **checker-side at `Expr` level** via `module.bindings` + `TypingForm::{Required, NotRequired, ReadOnly}` — `form.rs` already provides `is_typed_dict_qualifier()`.

Two of these (`model.rs:167,175`) are *mechanically* just threading `&module.bindings` into the now-2-arg `parse_static_condition`, but the file cannot compile until the qualifier model above is rebuilt, so I left them.

### C. `guards::is_stub_context` / `is_no_type_check` deleted — 5 errors, 4 files
`missing_parameter_annotation.rs:31` · `missing_return_annotation.rs:26` · `missing_vararg_annotation.rs:13` · `returns_compatibility.rs:27` (2 symbols)

`is_no_type_check` maps cleanly to `func.is_no_type_check` (verified real).

`is_stub_context` has no single flag. It can be rebuilt from `is_overload` + `is_stub_body` + class `is_protocol` — **but its `@abstractmethod` branch cannot be reproduced today** because no `is_abstractmethod` flag exists. Building it now would silently narrow the rule. **Wait for the resolver lane's `is_abstractmethod`.**

### D. `dataclass_transform` helpers deleted — 4 errors, 3 files
`dataclasses_frozen.rs:48,130` · `assignment_compatibility/dataclass_check.rs:50` · `constructors_call_init/mod.rs:370`

`collect_transform_classes` / `inherits_dataclass_transform` gone. A surviving variant exists in `dataclasses_transform_meta/helpers.rs` but with an **incompatible signature**. Needs reconciliation — **design decision required.**

### E. `decorator_spelled` — 3 errors, 2 files
See §3. **Blocked on resolver lane.**

### F. `annotation_is_type_alias` deleted — 2 errors, 2 files
`aliases_implicit.rs:33` · `shared/runtime_names.rs:23`

Depended on `insert_annotated_alias`, the PEP 613 `X: TypeAlias = ...` collector, also deleted. **Design decision required.**

### G. `protocols_generic::helpers::types_compatible` deleted — 1 error
`protocols_generic/helper_parity_tests.rs:7`

---

## 5. 52 golden tests that have never run

`tests/golden_typing_tests.rs` declared modules only up to `typeddicts`. Twelve files under `tests/golden/` were never registered, so Cargo never compiled or ran them — **CI could pass without executing any of them.** I registered all twelve:

| File | `#[test]` fns |
|---|---:|
| `directives_tests.rs` | 3 |
| `directives_more_tests.rs` | 2 |
| `literals_tests.rs` | 5 |
| `literals_more_tests.rs` | 4 |
| `override_tests.rs` | 4 |
| `self_type_tests.rs` | 5 |
| `specialtypes_tests.rs` | 6 |
| `typeddicts_keys_tests.rs` | 5 |
| `typeddicts_readonly_tests.rs` | 4 |
| `typeis_tests.rs` | 4 |
| `typevar_defaults_tests.rs` | 4 |
| `typevartuple_tests.rs` | 6 |
| **Total** | **52** |

> Note: the original review said "11 files / 46 tests". The true count is **12 / 52** — commit `ba48b251` added `directives_more_tests.rs` and rewrote `directives_tests.rs` after that review was written.
>
> `stdlib_*` golden files are **not** orphaned; they are registered by `golden_core_tests.rs`.

**Their pass/fail status is still unknown** — the crate does not compile, so they have not executed even once. Expect failures on first run; they were written against a checker that has since had its guards removed.

---

## 6. Changes made in this session

All in `crates/basilisk-checker` (no other lane touched).

**Registered the 12 orphaned golden modules** in `tests/golden_typing_tests.rs` (note `r#override` — reserved keyword).

**Swapped deleted spelling-matchers → resolved semantic flags** (zero new text matching):

| Deleted helper | Replaced with | Files |
|---|---|---|
| `is_enum_class(c)` | `c.is_enum` | 3 |
| `is_protocol_class(c)` | `c.is_protocol` | 3 |
| `is_namedtuple_class(c)` | `c.is_namedtuple` | 1 |
| `overload_decorated(resolver, &f.decorators)` | `f.is_overload` | 6 |
| `annotation_is_classvar(resolver, source, span)` | `attr.is_class_var` | 2 |

**Threaded `&module.bindings`** into the now-2-arg `parse_static_condition` in `enums_member_access.rs`.

**Cleaned the resulting cascade** — removed parameters that the flag swaps made dead (`resolver`, `source`) across `calls_argument_count/mod.rs` and `dataclasses_inheritance.rs`, converting two `let Some(resolver) = …else return` guards into explicit parse-error bails so the guard semantics survive; dropped two now-unused `AnnotationResolver` imports.

**Pruned** dead/dangling re-exports in `rules/shared.rs`.

**Stale reference to clean up:** `tests/checker/coverage_boost_25_tests.rs:509` names `split_top_level_args` in a comment; that function has now been deleted from the resolver.

---

## 7. Suggested order of work

1. **Unblock the build.** Categories C (minus abstractmethod), F, G, then A and D — those are the design decisions. Nothing in the checker can be tested until all 35 clear.
2. **Then run the 52 golden tests** for the first time and triage.
3. **Wait for the resolver lane** on `is_staticmethod`/`is_classmethod`/`is_abstractmethod`, then do categories C-abstractmethod and E in one pass.
4. **Rebuild category B checker-side** at `Expr` level — do not push it back into the resolver.
5. **Leave the 161 resolver failures failing** until the collectors are genuinely rebuilt on resolved identity. They are the honest map.

---

## 8. Conformance

Not run — `basilisk-checker` does not compile, so `cargo build --release` cannot produce a binary. Expect a **drop** once it does: the purge removed real detection capability (§2). Per CLAUDE.md that drop is **progress** and must be recorded plainly, not reversed. `coverage-thresholds.json` still gates at 100% pass / zero false positives, so `make test` will fail on that floor; removing the floor is the user's call.

---

## Appendix — all 161 failing tests

Reproduce a group with: `cargo test -p basilisk-resolver --test <target>`

<details><summary><code>tests/annotation_tests.rs</code></summary>

```
test_annotated::annotated_call_in_function
test_annotated::annotated_direct_call_at_module_level
test_annotated::annotated_direct_call_collected
test_annotated::annotated_subscript_call_collected
test_annotated::assert_type_strip_annotated
test_annotations::annotation_attribute_detected_as_any
test_assert_type::assert_type_in_class_method
test_assert_type::assert_type_in_except_handler
test_assert_type::assert_type_in_for_body
test_assert_type::assert_type_in_function_body
test_assert_type::assert_type_in_if_body
test_assert_type::assert_type_in_try_body
test_assert_type::assert_type_in_while_body
test_assert_type::assert_type_in_with_body
```

</details>

<details><summary><code>tests/class_tests.rs</code></summary>

```
test_dataclass::dataclass_transform_factory_marks_class
test_dataclass::dataclass_transform_field_init_false
test_dataclass::dataclass_transform_kw_only_default_flag
test_enum_class::enum_init_value_param_type
test_enum_class::enum_value_type_annotation_mismatch
test_enum_violations::enum_init_value_param_mismatch
test_enum_violations::enum_int_enum_also_detected
test_enum_violations::enum_value_type_mismatch_detected
```

</details>

<details><summary><code>tests/function_tests.rs</code></summary>

```
test_decorators::builtin_decorator_flags_do_not_depend_on_spelling
```

</details>

<details><summary><code>tests/misc_tests.rs</code></summary>

```
test_reveal_type::reveal_type_call_collected
test_reveal_type::reveal_type_calls_in_for_loop
test_reveal_type::reveal_type_calls_in_function
test_reveal_type::reveal_type_calls_in_try_except
test_reveal_type::reveal_type_calls_in_while_loop
test_reveal_type::reveal_type_calls_in_with_block
test_reveal_type::reveal_type_calls_only_matches_reveal_type
test_reveal_type::reveal_type_inside_match_collected
test_reveal_type::reveal_type_inside_try_collected
test_reveal_type::reveal_type_inside_while_collected
test_reveal_type::reveal_type_inside_with_collected
```

</details>

<details><summary><code>tests/mutant_tests.rs</code></summary>

```
test_coverage::dataclass_transform_attribute_form_decorator
test_coverage::dataclass_transform_bare_name_decorator
test_coverage::generic_with_subscript_non_typevar_arg
test_coverage::type_alias_subscript_rhs
test_coverage::type_alias_with_forward_ref
test_coverage::type_alias_with_tuple_rhs
test_mutant_annotation::annotation_flags_any_is_any
test_mutant_generic_params::extract_generic_params_collects_multiple_params
test_mutant_generic_params::extract_generic_params_single_param
test_mutant_special_calls::collect_special_calls_assert_type_returns_entries
test_mutant_special_calls::collect_special_calls_inside_class_def
test_mutant_special_calls::collect_special_calls_inside_for
test_mutant_special_calls::collect_special_calls_inside_function_def
test_mutant_special_calls::collect_special_calls_inside_if
test_mutant_special_calls::collect_special_calls_inside_match
test_mutant_special_calls::collect_special_calls_inside_try
test_mutant_special_calls::collect_special_calls_inside_while
test_mutant_special_calls::collect_special_calls_inside_with
test_mutant_special_calls::collect_special_calls_only_matches_exact_name
test_mutant_typeddict::collect_typeddict_calls_keyword_only_form
test_mutant_typeddict::collect_typeddict_calls_non_dict_second_arg
test_mutant_typeddict::collect_typeddict_calls_non_string_key_detected
test_mutant_typeddict::collect_typeddict_calls_only_matches_typeddict_name
test_mutant_typeddict::collect_typeddict_calls_qualified_typing_typeddict
test_mutant_typeddict::collect_typeddict_calls_returns_entries
test_mutant_typeddict::collect_typeddict_calls_string_keys_only
test_mutant_typevar::collect_typevar_calls_has_default_false_when_absent
test_mutant_typevar::collect_typevar_calls_has_default_true
test_mutant_typevar::collect_typevar_calls_plain_assign_arm
test_mutant_typevar::collect_typevar_calls_qualified_typing_typevar
test_mutant_visitor::collect_module_level_calls_from_assign
```

</details>

<details><summary><code>tests/protocol_tests.rs</code></summary>

```
test_protocol_01::abstract_class_instantiation_violation
test_protocol_01::protocol_classvar_attr_required
test_protocol_01::protocol_concrete_missing_method_violation
test_protocol_01::protocol_instantiation_detected
test_protocol_01::protocol_instantiation_in_class_body
test_protocol_01::protocol_instantiation_in_elif
test_protocol_01::protocol_instantiation_in_function_body
test_protocol_01::protocol_instantiation_in_if_body
test_protocol_01::protocol_instantiation_via_ann_assign
test_protocol_01::protocol_instantiation_via_expr_stmt
test_protocol_01::protocol_rtc_in_ann_assign
test_protocol_01::protocol_rtc_in_assign
test_protocol_01::protocol_rtc_in_class_def
test_protocol_01::protocol_rtc_in_elif
test_protocol_01::protocol_rtc_in_for_loop
test_protocol_01::protocol_rtc_in_function_def
test_protocol_01::protocol_rtc_in_while_loop
test_protocol_01::protocol_rtc_isinstance_tuple_arg
test_protocol_01::protocol_rtc_issubclass_data_protocol
test_protocol_01::protocol_rtc_not_runtime_checkable_isinstance
test_protocol_01::protocol_rtc_not_runtime_checkable_issubclass
test_protocol_01::protocol_transitive_required_methods
test_protocol_02::protocol_instantiation_violation_detected
test_readonly::readonly_kwargs_violation
test_readonly::readonly_kwargs_violation_detected
```

</details>

<details><summary><code>tests/resolution_tests.rs</code></summary>

```
test_control_flow::reveal_type_calls_in_if_body
```

</details>

<details><summary><code>tests/type_system_tests.rs</code></summary>

```
test_base_subscript_01::abstract_class_instantiation_detected_v2
test_base_subscript_01::multiple_unbounded_with_unpack_form
test_base_subscript_01::readonly_kwargs_subscript_assign
test_base_subscript_01::typeddict_bool_literal_for_str_field
test_base_subscript_01::typeddict_dict_literal_wrong_value_type
test_base_subscript_01::typeddict_none_literal_value
test_base_subscript_01::typeddict_subscript_assign_wrong_int_for_str
test_base_subscript_02::abstract_class_instantiation_subscript
test_base_subscript_02::non_typevar_expr_in_generic_brackets
test_base_subscript_02::protocol_class_factory_instantiation
test_base_subscript_02::protocol_instantiation_subscript
test_base_subscript_02::starred_typevartuple_generic_param
test_base_subscript_02::typeddict_float_literal_for_int_field
test_base_subscript_03::assert_type_annotated_param
test_base_subscript_03::direct_tuple_index_check_does_not_depend_on_annotation_spelling
test_base_subscript_03::enum_value_bytes_vs_str
test_base_subscript_03::enum_value_float_vs_int
test_base_subscript_03::enum_value_none_vs_str
test_base_subscript_03::key_lambda_tuple_index_check_does_not_depend_on_annotation_spelling
test_base_subscript_03::literal_string_enum_single_quote_form
test_base_subscript_03::namedtuple_list_second_arg_form
test_base_subscript_03::protocol_isinstance_non_rtc_module
test_base_subscript_03::readonly_annotation_via_typing_attribute
test_base_subscript_03::readonly_binop_annotation
test_base_subscript_03::stub_body_pass_statement
test_base_subscript_03::type_alias_type_ann_assign
test_base_subscript_03::type_alias_type_call_info
test_base_subscript_03::typeddict_functional_form_dict
test_base_subscript_03::typeddict_subscript_wrong_value_type
test_literal_enum::literal_string_enum_mismatch_found
test_newtype::newtype_call_collected
test_newtype::newtype_call_int_base
test_type_alias::type_alias_def_explicit_annotation
test_type_alias::type_alias_string_refs_collected
test_type_alias::type_alias_with_binop_rhs
test_type_alias::type_alias_with_subscript_rhs
```

</details>

<details><summary><code>tests/typeddict_tests.rs</code></summary>

```
test_namedtuple::namedtuple_collections_form_comma_string
test_namedtuple::namedtuple_collections_form_list_arg
test_namedtuple::namedtuple_collections_form_string_arg
test_namedtuple::namedtuple_collections_form_tuple_arg
test_namedtuple::namedtuple_collections_list_of_strings
test_namedtuple::namedtuple_collections_string_arg
test_namedtuple::namedtuple_final_string_constant_resolved
test_namedtuple::namedtuple_functional_3_fields
test_namedtuple::namedtuple_typing_form_collected
test_namedtuple::namedtuple_typing_form_tuple_pairs
test_namedtuple::namedtuple_typing_list_of_pairs
test_namedtuple::namedtuple_typing_tuple_of_pairs
test_namedtuple::namedtuple_with_defaults_keyword
test_typeddict_calls::typeddict_functional_call_collected
test_typeddict_calls::typeddict_regular_assign_full_check
test_typeddict_keys_01::assert_type_with_literal_args
test_typeddict_keys_01::protocol_instantiation_in_except_handler
test_typeddict_keys_01::protocol_instantiation_in_finally_block
test_typeddict_keys_01::protocol_instantiation_in_orelse_block
test_typeddict_keys_01::protocol_instantiation_in_try_block
test_typeddict_keys_01::protocol_instantiation_in_with_block
test_typeddict_keys_01::protocol_instantiation_subscript_call
test_typeddict_keys_01::typeddict_value_type_check_does_not_depend_on_builtin_spelling
test_typeddict_keys_01::typeddict_wrong_value_type_regular_assign
test_typeddict_keys_01::typeddict_wrong_value_type_subscript_assign
test_typeddict_keys_01::types_match_quoted_forward_ref
test_typeddict_keys_02::enum_value_init_param_type_mismatch
test_typeddict_keys_02::is_enum_member_simple_form
test_typeddict_keys_02::protocol_rtc_in_class_body
test_typeddict_keys_02::protocol_rtc_in_for_body
test_typeddict_keys_02::readonly_kwargs_subscript_assign_violation
test_typeddict_keys_02::typeddict_delete_notrequired_key_inside_function_ok
test_typeddict_keys_02::typeddict_subscript_assign_wrong_type
test_typeddict_keys_02::types_match_bare_generic_vs_any
```

</details>

