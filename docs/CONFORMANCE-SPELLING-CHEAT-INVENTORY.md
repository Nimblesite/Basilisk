# [AUDIT-SPELLING-INVENTORY] Spelling-Recognition Cheat Inventory (Phase 2)

**Date:** 2026-08-06. Companion to [`docs/CONFORMANCE-INTEGRITY-AUDIT.md`](CONFORMANCE-INTEGRITY-AUDIT.md).
**Status:** DELETION IN PROGRESS — every site below is under a mandated deletion order. This
document is the permanent record of the pre-deletion inventory; line numbers are as surveyed
on this date and will not match post-deletion sources.

## The finding

Phase 1 deleted the checker code fitted to the exact **text** of the python/typing conformance
fixtures. Phase 2 surveyed the remaining recognition machinery and found the deeper layer: the
checker and resolver "recognise" typing special forms by comparing **source-text spellings**
against hardcoded strings, instead of resolving imports. A fixture that writes
`from typing import ClassVar as CV` (sharkdp's AST-preserving mutation harness, vendored at
`conformance/mutate_typing_conformance.py`) defeats every one of these sites — which is why the
pristine suite scored 141/141 while the mutated suite scored 32/141 (22.7%).

**Mandate (project owner, 2026-08-06): DELETE all of it, wholesale, before any replacement is
built. The code does not need to compile and the tests do not need to pass during deletion.
Only after every trace is deleted may recognition be rebuilt on import resolution.**

## The legitimate mechanism (NOT cheat — kept)

`AnnotationResolver::spelling_denotes_from` → `canonical_head`/`imported_leaf` rewrite a source
spelling into its **resolved** member name via the module's import table. Files that go through
this cascade are legitimate:

- `crates/basilisk-checker/src/annotation/mod.rs`, `annotation/forms.rs`, `annotation/builtins.rs`
- `crates/basilisk-checker/src/rules/shared/typing_form.rs` (`denotes`, `denotes_abc`,
  `subscript_of`, `denotes_form`, `decorator_denotes`)
- Rules calling those helpers (~59 call sites at survey time). A member-name string literal
  passed to a cascade function is compared against resolved import targets — that is fine.

Also NOT cheats: keyword-argument names at call sites (`"bound"`, `"kw_only"`, `"total"`, …);
true builtins needing no import (`int`, `str`, `isinstance`, `object`, …); dunder names;
diagnostic message text; test code.

## Cheat mechanism classes

| Class | Pattern | Example |
|---|---|---|
| S1 | AST name/attr spelling match | `name.id.as_str() == "TypeVar"` |
| S2 | Rendered-annotation-text match | `ann.starts_with("Callable[")`, `ty == "Any"` |
| S3 | Decorator/base-name spelling gate | `decorator_spelled(d, "overload")`, `bases.iter().any(\|b\| b == "Protocol")` |
| S4 | Const arrays of import-requiring spellings | `SPECIAL_FORMS`, `KNOWN_PROTOCOLS`, `BUILTIN_TYPE_NAMES` |
| S5 | Raw source-line scanning (no AST) | `source_line.find("Generic[")` |

Three shared helpers are pure spelling comparators with no resolver access — every call site
passing an import-requiring symbol is a cheat:

- `crates/basilisk-checker/src/rules/shared.rs:58` `decorator_spelled` (`d == name || d.rsplit('.').next() == Some(name)`)
- `crates/basilisk-resolver/src/visitor/walks.rs:47` `is_name_or_attr_named` (`n.id.as_str() == target`)
- `crates/basilisk-checker/src/rules/shared.rs:154` `ann_str` renders raw source spelling; any
  `== "X"` comparison on its result is spelling recognition (the renderer itself is not a cheat).

`ClassInfo.bases` (`crates/basilisk-resolver/src/scope/class_types.rs:64`) stores **raw source
text** ("Base class names (simple names only)"), so every `bases` comparison against a typing
symbol is spelling recognition.

## Layer 1 — basilisk-resolver visitors (~154 sites, 26 files)

The resolver's visitor layer recognises special-form calls by callee spelling
(`expr_simple_name(&call.func) == Some("TypeVar")` and kin), so renamed imports empty
`typevar_calls`/`newtype_calls`/`typeddict_calls`/etc. and every downstream consumer loses its
inputs. Files (all under `crates/basilisk-resolver/src/`):

`visitor/annotations.rs`, `visitor/assert_narrow.rs`, `visitor/assigns.rs`,
`visitor/call_return.rs`, `visitor/calls_and_reveal.rs`, `visitor/class_info.rs`,
`visitor/class_info_ext.rs`, `visitor/core.rs`, `visitor/dataclass.rs`,
`visitor/enum_checks.rs`, `visitor/final_readonly.rs`, `visitor/generics.rs`,
`visitor/mod.rs`, `visitor/module_level.rs`, `visitor/narrowing.rs`,
`visitor/pep695_scoping.rs`, `visitor/protocol.rs`, `visitor/protocol_ext.rs`,
`visitor/type_alias.rs`, `visitor/typeddict.rs`, `visitor/typeddict_ext.rs`,
`visitor/typevar.rs`, `visitor/unhashable.rs`, `visitor/walks.rs`, `visitor/walrus.rs`,
`scope/function_types.rs`.

Recognised-by-spelling symbols include: `TypeVar`, `TypeVarTuple`, `ParamSpec`, `TypedDict`,
`TypeAliasType`, `NewType`, `NamedTuple`, `Protocol`, `Generic`, `Final`, `ClassVar`,
`assert_type`, `reveal_type`, `cast`, `dataclass`, `Enum`-family, `overload`, `override`,
`final`, `deprecated`, `total_ordering`, and more.

## Layer 2 — basilisk-checker production code (~277 sites, ~95 files)

Survey method: enumerated every quoted typing/typing_extensions/dataclasses/enum/abc/functools/
collections.abc symbol as a string literal plus bracket-prefix forms across
`crates/basilisk-checker/src`, read each hit in context, classified against the cascade.
Only 15 of 309 production files touch the legitimate cascade; everything below bypasses it.

### Core type machinery (non-rules)

- `src/types_parsing.rs` — pure text parser, no import view: 32 (`"any"|"object"|"final"|"tuple"|"type"` → Any), 34 (`"never"`), 36 (`"literalstring"`), 39 (`"callable"`), 78 (`strip_subscript("literal[")`), 81 (`callable[`…`]`), 85 (`generator[`), 97/100 (`typeform`/`typeform[`), 104 (`final[`), 108 (`union[`), 158 (`optional[`)
- `src/subtyping.rs` — 130 (`sub == "Any" || sup == "Any" || sup == "object"` on annotation text)
- `src/tyeval/lower.rs` — 188–193 (`("Union"|"typing.Union")`, `("Optional"|…, 1)`, `("Annotated"|…)` on `dotted_text()` raw spelling)
- `src/annotation/tables.rs` — 286 (`leaf == "Protocol" || leaf == "TypedDict"`), 293 (`name == "TypeAlias" || name.ends_with(".TypeAlias")`) — the table builder itself
- `src/exports.rs` — 699 (`head != "object" && head != "Any"`)
- `src/stub_constructor.rs` — 113 (same filter), 247 (`mro.iter().any(|n| n == "Any")`)

### Rules — direct AST identifier/attribute spelling (S1)

- `rules/protocols_class_objects_2.rs`: 159, 161 (`== "Protocol"`), 251 (`== "ClassVar"`)
- `rules/callables_kwargs.rs`: 579 (`== "TypedDict"`), 588 (`== "TypeVar"`), 593 (`== "Unpack"`)
- `rules/aliases_implicit.rs`: 413 (`== "Concatenate"`)
- `rules/literals_semantics.rs`: 114 (`== "Literal"`)
- `rules/protocols_explicit_2.rs`: 112 (`== "abstractmethod"`)
- `rules/namedtuples_usage.rs`: 141–142 (`== "NamedTuple"` name + attr)
- `rules/generics_self_basic.rs`: 109 (`ann_text.trim() != "Self"`), 240 (`!= "Self"`)
- `rules/assignment_compatibility/callable_check.rs`: 60 (`== "TypeAlias"`), 73 (`== "ParamSpec"`), 161 (`== "Callable"`), 217 (`strip_prefix("Concatenate[")`)
- `rules/assignment_compatibility/sig_model.rs`: 103–104 (`"Protocol"`/`"Generic"` arms), 123 (`== "overload"`), 173 (`ty == "Any"`)
- `rules/assignment_compatibility/skip_names.rs`: 118 (`== "TYPE_CHECKING"`)
- `rules/callables_protocol/mod.rs`: 161, 228, 250, 269 (`is_name_or_attr_named` "Callable"/"Concatenate")
- `rules/callables_protocol/hof_paramspec.rs`: 175 (`ann_str == "Callable"`), 188 (`== "Concatenate"`)
- `rules/callables_protocol_2/checks.rs`: 162 (`== "cast"`), 314 (`starts_with("Callable[")`)
- `rules/callables_protocol_2/context.rs`: 177, 179 (`== "Protocol"`), 244 (`== "overload"`), 334 (`== "TypedDict"`), 382–384 (`"Required"`/`"NotRequired"`/`"ReadOnly"` arms), 443 (`strip_prefix("Unpack[")`)
- `rules/callables_protocol_2/callable.rs`: 40 (`Callable[`), 55 (`Concatenate[`), 252 (`== "Any"`)
- `rules/directives_deprecated/decorators.rs`: 28, 30, 48, 50 (`== "deprecated"`)
- `rules/directives_deprecated/collect.rs`: 43–44 (`== "overload"`)
- `rules/directives_deprecated/visit_expr.rs`: 387 (`kind != "overload"`)
- `rules/dataclasses_transform_meta/helpers.rs`: 93, 102 (`== "dataclass_transform"`)
- `rules/dataclasses_transform_class/converter.rs`: 97 (`== "dataclass_transform"`), 384 (`== "overload"`)
- `rules/specialtypes_type/helpers.rs`: 7–31 (`SPECIAL_FORMS` const), 115 (`== "TypeVar"`)
- `rules/specialtypes_type/mod.rs`: 100 (`== "TypeAlias"`), 383 (`SPECIAL_FORMS.contains`)
- `rules/annotations_forward_refs/mod.rs`: 137 (`"Protocol" | "Generic"`), 236–237 (`== "TypeAlias"`)
- `rules/annotations_forward_refs/type_checks.rs`: 29 (`subscript_base_is "Annotated"`), 33 (`== "Generic"`), 134 (`== "Concatenate"`), 140 (`== "Callable"`)
- `rules/typeddicts_extra_items/model.rs`: 82–85 (`ReadOnly[`/`Required[`/`NotRequired[` prefixes), 133, 156, 287 (`== "TypedDict"`)

### Rules — bases/decorators/base_name raw-spelling comparisons (S3)

- `rules/guards.rs` — **shared gating predicates; the cheat propagates to every caller**: 28 (`decorator_spelled "overload"`), 37 (`"abstractmethod"`), 55 (`"no_type_check"`), 67 (`"Enum"|"IntEnum"|"StrEnum"|"Flag"|"IntFlag"|"ReprEnum"` with `strip_prefix("enum.")`), 81 (`bases == "Protocol"`), 90 (`bases == "NamedTuple"`), 112 (`== "dataclass_transform"`), 193 (`decorator_spelled "dataclass_transform"`)
- `rules/protocols_merging.rs`: 29 (`ALLOWED_BASES`), 32–63 (`KNOWN_PROTOCOLS` — Sized, Hashable, Iterable, …), 74, 86, 98
- `rules/protocols_definition.rs`: 52; `rules/protocols_subtyping.rs`: 247; `rules/protocols_modules.rs`: 57; `rules/protocols_variance_2.rs`: 158; `rules/protocols_runtime_checkable_2.rs`: 186, 196 (`"runtime_checkable"`)
- `rules/protocols_definition_2/mod.rs`: 66 (`"Callable" => ["__call__"]`), 119, 180, 277, 282, 304, 320
- `rules/protocols_definition_2/call_args.rs`: 274, 359, 437, 462
- `rules/protocols_definition_2/conformance.rs`: 20 (`ClassVar[` prefixes), 77 (`"NamedTuple"`)
- `rules/protocols_generic/mod.rs`: 131, 135, 144, 151, 155, 226, 230, 385, 389 (`"Protocol"`/`"Generic"`)
- `rules/protocols_generic/helpers.rs`: 89, 94, 308, 368, 487 (`== "Any"`/`"Self"` on text)
- `rules/namedtuples_define_class.rs`: 62, 109 (`ClassVar` prefix), 150, 231, 232, 239, 262
- `rules/namedtuples_type_compat.rs`: 84
- `rules/typeddicts_inheritance.rs`: 143 (`EXEMPT`), 145, 151, 295
- `rules/classes_override.rs`: 89; `rules/classes_override_3.rs`: 43; `rules/missing_override_decorator.rs`: 167, 189 (`"override"`/`"overload"`)
- `rules/overloads_definitions.rs`: 97; `rules/overloads_consistency_2.rs`: 161, 185 (`["final","override"]`); `rules/overloads_consistency_3.rs`: 41–46
- `rules/qualifiers_final_decorator.rs`: 30 (`== "final"`)
- `rules/enums_members_2.rs`: 109 (`Literal[`), 159 (`decorator_spelled "member"`); `rules/enums_expansion.rs`: 105
- `rules/constructors_call_type/helpers.rs`: 52, 108, 143, 301
- `rules/constructors_call_init/helpers.rs`: 57, 93–94, 127, 202, 251, 366 (`contains("Self")`)
- `rules/constructors_call_init/mod.rs`: 122, 320
- `rules/calls_argument_count/mod.rs`: 291 (`== "Self"`), 601, 612
- `rules/calls_argument_count/method_binding.rs`: 22–29 (`SIGNATURE_PRESERVING` — `overload`/`override`/`final`/`abstractmethod` entries)
- `rules/calls_argument_type/mod.rs`: 282
- `rules/generics_base_class_2.rs`: 246; `rules/generics_syntax_compatibility.rs`: 107
- `rules/shared.rs`: 58 (`decorator_spelled` itself), 218 (`find("Literal[")`), 317 (`!= "Protocol" && != "Generic"`)

### Rules — annotation-text substring/prefix matching (S2)

- `rules/typeddicts_required.rs`: 41, 46–49 (`contains("Required[")` nests)
- `rules/classes_override_2.rs`: 140–141 (`contains("ClassVar")`), 170 (`contains("ReadOnly"/"Required"/"NotRequired")`)
- `rules/redundant_annotation.rs`: 135–137 (`contains("TypedDict"/"Protocol"/"NamedTuple")`), 346
- `rules/qualifiers_final_annotation_2.rs`: 65–70 (`Final`/`Final[`/`ClassVar[Final`/`ClassVar[typing.Final` prefixes)
- `rules/qualifiers_annotated_2.rs`: 40 (`find("Annotated[")`)
- `rules/qualifiers_annotated/helpers.rs`: 19–91 (`BUILTIN_TYPE_NAMES` treats `Union`, `Optional`, `Callable`, `ClassVar`, `Final`, `Literal`, `Annotated`, `TypeVar`, `TypeVarTuple`, `ParamSpec`, `Generic`, `Protocol`, `TypedDict`, `NamedTuple`, `NewType`, `TypeAlias`, `Never`, `NoReturn`, `Self`, `LiteralString`, `Unpack`, `Required`, `NotRequired`, `ReadOnly`, `Concatenate` as always-in-scope regardless of imports)
- `rules/annotations_forward_refs/scope.rs`: 71–156 (`PYTHON_BUILTIN_TYPE_NAMES` — same, plus `overload`, `cast`, `assert_type`, `reveal_type`)
- `rules/literals_parameterizations.rs`: 59, 64, 79 (`Literal`/`.Literal[` forms)
- `rules/generics_type_erasure.rs`: 137; `rules/generics_scoping.rs`: 296, 374 (`== "TypeAlias"`)
- `rules/generics_self_usage.rs`: 76 (**byte-scans source for `b"Self"`**), 174, 265, 289
- `rules/generics_self_attributes.rs`: 141
- `rules/generics_upper_bound.rs`: 297 (`contains("Protocol")`); `rules/generics_upper_bound_2.rs`: 39
- `rules/generics_basic.rs`: 45–49 (callee `"TypeVarTuple"`/`"ParamSpec"`/`"TypeVar"`)
- `rules/generics_basic_3/helpers.rs`: 254, 362 (`Generic[`), 433 (`== "Any"`)
- `rules/generics_typevartuple_basic_3.rs`: 39 (`callee != "TypeVarTuple"`)
- `rules/generics_typevartuple_callable.rs`: 261, 286 (`Callable[`)
- `rules/generics_defaults_referential_2_helpers.rs`: 68, 72 (`rhs.starts_with("TypeVar(")` — RHS source text)
- `rules/generics_defaults_referential_2.rs`: 200 (`find("Generic[")`)
- `rules/generics_defaults_specialization.rs`: 281 (`Concatenate[`)
- `rules/callables_annotation.rs`: 193; `rules/callables_subtyping.rs`: 75, 209 (`Callable[`)
- `rules/constructors_callable.rs`: 148 (`Callable[`/`.Callable[`)
- `rules/constructors_callable/conversion.rs`: 135, 170–171 (`Union[`/`typing.Union[`), 185 (`== "Self"`)
- `rules/overloads_basic.rs`: 245 (`== "Any"`), 262 (`Union[`), 270 (`Optional[`)
- `rules/overloads_evaluation.rs`: 379, 473 (`Union[`)
- `rules/dataclasses_postinit.rs`: 68 (`dataclasses.InitVar[`)
- `rules/names_undefined.rs`: 141 (`callee == "reveal_type"`)
- `rules/specialtypes_never.rs`: 94–95 (`"NoReturn" | "Never"` spelling arms)
- `rules/classes_classvar/helpers.rs`: 37–39 (`TypeVar`/`ParamSpec`/`TypeVarTuple` spellings fed into a source-name comparison)
- `rules/assignment_compatibility/typeform_check.rs`: 24 (`INVALID_TYPE_FORMS`), 27 (`REQUIRES_PARAMETERISATION`), 250, 255
- `rules/assignment_compatibility/typeddict_struct.rs`: 125, 127, 170
- `rules/assignment_compatibility/sig_subtype.rs`: 258; `rules/assignment_compatibility/alias_match.rs`: 235; `rules/assignment_compatibility/mod.rs`: 300 (`== "typealias"` lowercased)
- `rules/calls_argument_type/builtin_methods.rs`: 145, 151 (`contains("LiteralString")`)
- `rules/annotations_generators_2/type_check.rs`: 132, 303 (`== "Any"`)
- `rules/tuples_type_compat/annotation.rs`: 291 (`== "Any"`)

### Rules — raw source-line scanning (S5, worst class: no AST at all)

- `rules/generics_variance_inference/mod.rs`: 294–295 (`line.contains("TypeAlias") && !contains("TypeAliasType")`), 351–354 (`line.contains("TypeVar"/"ParamSpec"/"TypeVarTuple"/"TypeAlias")`)
- `rules/generics_variance_inference/variance.rs`: 148 (`Generic[` prefix), 162 (`prev.contains("dataclass")`), 236, 337 (`ann.contains("Final")`), 355 (`Generic[`/`Protocol[`)
- `rules/generics_variance_inference/collect.rs`: 24 (`line.starts_with("class ") && contains("Generic[")`)
- `rules/generics_variance_inference/utils.rs`: 198, 200 (`source_line.find("Generic[")`/`"Protocol["`)

## Totals

| Layer | Sites | Files |
|---|---|---|
| basilisk-resolver visitors/scope | ~154 (`expr_simple_name` uses; recognition subset thereof) | 26 |
| basilisk-checker production | ~277 | ~95 |

Excluded from the counts (verified non-cheat): the ~59 cascade call sites, kwarg-name matches,
builtins, dunders, `Display` impls, diagnostic message text, and all test code.

## Deletion execution (2026-08-06)

Two multi-agent deletion waves, each followed by two independent adversarial verifiers and a
cleanup round:

1. **Wave 1 — resolver**: 10 agents over the 26 resolver files.
2. **Wave 2 — checker**: 12 agents over the ~95 checker files, batched by rule family.

Deletion rules of engagement: deletion only, no reimplementation, no stubs fabricating
verdicts, no spelling match kept to preserve compilation, test code untouched (every failing
test is kept failing). After deletion: the workspace is brought back to building by **deleting
dead fallout only**, then the pristine suite (`conformance/run_conformance.py --ref a490662`)
and mutated suite (`conformance/run_mutation_conformance.py`) are re-run and the honest scores
recorded, however low. Rebuilding recognition on the import-alias table
([TYPEINF-ANNOTATION-RESOLUTION] cascade, extended into basilisk-resolver) is the ONLY
sanctioned path back up.

### Deletion progress

| Layer | State | Evidence |
|---|---|---|
| basilisk-resolver | **CLEAN** | Automated sweep of every production `.rs` file (test modules excluded) for all import-requiring symbol spellings returns zero non-comment hits. Four mentions survive, all inside doc comments. `is_name_or_attr_named`, `strip_typeddict_qualifiers`/`try_strip_wrapper`, `builtin_annotation`'s `LiteralString` arm, the `LiteralString` receiver filter, `ClassVar`-prefix dataclass-field detection, and the `TYPE_CHECKING`/`sys.version_info` static-condition parsers are all gone, along with their re-exports. |
| basilisk-checker | IN PROGRESS | Deletion wave running; sweep count falling. |

Failing tests left failing by design: `static_condition.rs`'s `TYPE_CHECKING` / `sys.version_info`
parse tests, and every other test that asserted on deleted recognition. Per the standing rule,
**no failing test is deleted, weakened, or ignored** — they are the honest record of what the
checker can no longer do until recognition is rebuilt on import resolution.

## Reference scores (pre-deletion, 2026-08-06, fresh runs)

| Suite | Score | Meaning |
|---|---|---|
| Pristine (official harness @ a490662) | 141/141, 0 missed, 0 FP | Carried by the spelling layer above |
| Mutated (sharkdp harness: 527 renames + 729 reformats) | 32/141 (22.7%) | Observed mutated-fixture pass rate; not a conformance percentage |

Neither row states Basilisk's current conformance level, which remains
temporarily unknown. The 109-file gap between the two historical fixture runs
is exactly what this inventory explains.
