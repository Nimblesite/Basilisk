# Which PEP rules actually work — 2026-08-09

## The answer

**20 of 117 PEP rules decide correctly and keep deciding correctly when the
source is respelled. None of them were validated by the existing test suite —
all 20 were proven by a corpus written today, and every one of them is
file-local: it goes silent the moment the symbol it keys on arrives through a
re-export.**

I probed 54 of the 117 rules with freshly written Python that appears nowhere in
`conformance/tests`. Each rule got four variants of the same program —
ill-typed, ill-typed with aliased imports and renamed identifiers, ill-typed
reformatted, and well-typed — plus, where applicable, a lookalike-shadow
variant and a one-hop re-export variant.

| Result over the 54 probed | Count |
|---|---|
| Correct verdict, survives every mutation | 20 |
| Wrong verdict (false positive or false negative) | 7 |
| Inert — no diagnostic on ill-typed input | 27 |
| Not probed (of the full 117) | 63 |

## The 20 that work

Each fires on ill-typed Python, stays silent on well-typed Python, and returns
an identical verdict under aliased imports (`from enum import Enum as Grouping`,
`import typing as _t`), reformatting, identifier renaming, and — where the rule
keys on a stdlib symbol — a locally-defined lookalike class of the same name.

| Rule | Obligation |
|---|---|
| `aliases_implicit` | invalid RHS for a `TypeAlias` annotation |
| `aliases_type_statement` | invalid RHS in a PEP 695 `type X = ...` |
| `annotations_typeexpr` | numeric literal used as a type annotation |
| `calls_argument_count` | too few arguments at a call site |
| `dataclasses_inheritance` | non-default field after a defaulted field |
| `dataclasses_match_args` | `__match_args__` on `match_args=False` |
| `dataclasses_slots` | `slots=True` colliding with a manual `__slots__` |
| `enums_behaviors` | subclassing an Enum that has members |
| `enums_members` | Enum member carrying an explicit annotation |
| `generics_basic` | `TypeVar` declared with exactly one constraint |
| `historical_positional` | `__name` positional-only parameter passed by keyword |
| `imports_unresolved` | unresolved import |
| `names_undefined` | reference to a name with no visible definition |
| `overloads_consistency_2` | inconsistent decorators across an overload set |
| `overloads_definitions` | missing `@overload` implementation |
| `qualifiers_final_decorator` | subclassing a `@final` class |
| `returns_compatibility` | return value incompatible with the annotation |
| `typeddicts_class_syntax` | method defined in a `TypedDict` body |
| `typeddicts_class_syntax_2` | invalid keyword in a `TypedDict` class header |
| `typeddicts_readonly` | mutation of a `ReadOnly` field |

### The ceiling on all 20

Every one of them resolves symbols **within a single file only**. Given

```python
# shim.py
from enum import Enum as Grouping

# use.py
from shim import Grouping
class Tide(Grouping):
    NEAP: int = 1        # enums_members must fire here
```

the import resolves (no `imports_unresolved`), and the rule emits nothing.
Same for `dataclasses_*`, `typeddicts_*`, `overloads_*`,
`qualifiers_final_decorator`, `generics_basic`, `calls_argument_count`, and
`classes_override_2`. `os.getcwd(1, 2, 3)` also draws nothing, so this is not
about local modules — the checker does not carry symbol identity or signatures
across a module boundary at all. Treat all 20 as *file-local* rules.

## The 7 that produce wrong verdicts

Measured, each against a program a conforming checker must judge the other way.

| Rule | Defect | Trigger |
|---|---|---|
| `overloads_consistency` | **false positive on every correct overload set** | `sound(int)` / `sound(str)` reported as "same parameter signature" — it compares `annotation_text`, which the resolver always sets to `None`, so any same-arity pair compares equal |
| `match_exhaustiveness` | **false positive** | a `match` naming every member of an enum is reported non-exhaustive for lacking `case _:`, which the spec does not require |
| `classes_override_2` | **false positive on respelling** | base `x: int`, child `x: "int"` or `x: builtins.int` — same type, reported as an incompatible override; the verdict is `child_ann != base_ann` on raw annotation slices |
| `dataclasses_frozen` | **false negative on respelling** | fires on `b = P(1.0)`, silent on `b = (P)(1.0)` and silent for a parameter annotated `P`; the instance→class map is sliced out of the assignment's RHS text |
| `calls_argument_type` | **false negative on respelling** | rejects `str` against `int \| float`, accepts it against `Union[int, float]` |
| `tuples_type_form` | **both** | silent on `typing.Tuple[int, ..., str, ...]`; fires on a locally-defined class named `tuple` |
| `typeddicts_usage` | **false negative on respelling** | rejects `isinstance(x, TD)`, accepts it when `isinstance` is reached through a local alias |

`generics_defaults_specialization` fires against a hand-rolled local class named
`Generic`, so it is keyed to that spelling; it is excluded from the 20 for that
reason.

`assignment_compatibility` accepts a list, dict, or set literal assigned to a
variable annotated with a *user-defined* class named `List`, `Dict`, or `Set`.
Renaming that class to `Widget` produces the error correctly. This is the
lowercase-and-string-match parser deleted today.

## The 27 measured inert

Given ill-typed Python that directly violates the obligation each one is
documented to enforce, no diagnostic was produced:

`aliases_newtype`, `aliases_recursive`, `annotations_generators`,
`classes_classvar`, `dataclasses_kwonly`, `dict_key_hashable`,
`directives_assert_type`, `directives_assert_type_2`, `directives_cast`,
`directives_deprecated`, `directives_reveal_type`, `generics_base_class`,
`generics_basic_2`, `generics_syntax_compatibility`, `generics_variance`,
`literals_parameterizations`, `namedtuples_define_functional`,
`narrowing_typeguard`, `narrowing_typeis_2`, `protocols_explicit`,
`protocols_merging`, `protocols_runtime_checkable`,
`qualifiers_final_annotation_2`, `specialtypes_never`, `tuples_index`,
`typeddicts_inheritance`, `version_target_syntax`.

Some of these are basic: instantiating a `Protocol`, `isinstance` against a
non-`runtime_checkable` Protocol, a `-> NoReturn` function that falls through,
a tuple index out of bounds, `cast(1, "hello")`, a bare `reveal_type()`.

## What the existing test suite proves about all this: nothing

- `assert_rejected_by` — the only assertion that attributes a diagnostic to a
  named rule — is used **zero** times.
- `typing_spec_obligations`, the largest suite at 1,586 tests, contains **1,379
  no-assertion bodies and 33 real assertions**. Its own header says it was
  written to raise a coverage number.
- Repo-wide there are **1,593 no-assertion bodies across 121 files**
  (`let _ = run(source)?`, `let _ = codes(&diags)`).
- Not one of the 20 working rules above was established by an existing test.
  They were established today by `scratchpad`-generated Python run through the
  release binary, and the wrong verdicts in the table above were sitting under a
  green suite.

`tests/no_assertion_debt_tests.rs` fails on that count and names the worst
offenders. It is satisfied only by writing assertions.

## Deleted today

Three cross-cutting helpers that decided types from source text. All were
deleted outright; none was replaced, and no call site was repaired.

| Deleted | What it did |
|---|---|
| `src/types_parsing.rs` | `InferredType::from_annotation` — lowercased annotation text, then matched `"int"`, `"str"`, `"object"`, `"list["`, `"dict["`, `"tuple["`, split on `\|` |
| `src/subtyping.rs` | `name_subtype` / `SubtypingContext::is_subtype` — a subtyping judgement over `&str` annotation text, `sup == "object"`, `sub.contains('\|')` |
| `src/types_star_tuples.rs` | re-read the above parser's output as text: `Named("...")`, `Named(name).starts_with('*')`, `strip_prefix("*tuple[")` |

**`basilisk-checker` does not compile.** 32 call sites across 16 files are left
broken on purpose — they are the map of what must be rebuilt on resolved
bindings, canonical `TypeNode`, and `assignable`/`equivalent`, or made to
abstain. Each of those files now carries a `BROKEN` banner naming the deleted
helper it called and forbidding its restoration:

`incremental_defs.rs`, `narrow/flow.rs`, `narrow/guards.rs`, `param_infer.rs`,
`tyeval/lower.rs`, `types.rs`, `rules/assignment_compatibility/{mod,alias_match,callable_check}.rs`,
`rules/generics_defaults_2.rs`, `rules/generics_defaults_referential.rs`,
`rules/generics_syntax_scoping/alias_misuse.rs`, `rules/narrowing_typeis_2.rs`,
`rules/overloads_evaluation.rs`, `rules/shared/{judge,module_types}.rs`.

Do not restore, vendor, or re-derive any of it. A checker that does not build is
recoverable; a checker that answers from spelling is not.

## Tests added today

Both files fail. Both are the specification of correct behaviour, measured
against the release binary before the deletion.

- `tests/legacy_annotation_text_parser_pin_tests.rs` — renaming a user class
  from `Widget` to `List`/`Dict`/`Set`/`Int`/`Str` must not change any
  diagnostic. 4 of 6 fail.
- `tests/pep_spelling_invariance_pin_tests.rs` — one test per wrong verdict in
  the table above: quoted and qualified annotations denote the same type,
  `Union[A, B]` equals `A | B`, `(P)(x)` equals `P(x)`, overloads differing in
  parameter type are distinct, an enum-complete `match` is exhaustive, an
  aliased `isinstance` is still `isinstance`, `typing.Tuple` is `tuple`.

## Still text-matched, not yet deleted

Rule-local text verdicts found by audit and not touched today. Each decides
whether to emit by reading source text:

`specialtypes_type` (`strip_prefix("type[")` + builtin-name whitelist; union
members split on the literal `" | "`), `assignment_compatibility`
(tuple/dataclass/typeform checks slice the source line), `calls_argument_count`
(`is_clearly_incompatible` splits annotations on `|` against a scalar
whitelist), `overloads_basic`, `overloads_evaluation`,
`generics_typevartuple_basic_2` (`ann.find("tuple[")`),
`generics_typevartuple_specialization`, `generics_typevartuple_unpack`
(`ann.contains("*tuple[")`), `generics_syntax_compatibility` (substring-scans
annotations for the TypeVar name), `protocols_variance` (covariant-container
whitelist), `constructors_call_init`, `redundant_annotation`,
`generics_syntax_scoping`, `annotations_forward_refs`, `generics_defaults_2`,
`generics_defaults_referential`, `calls_argument_type`, `dataclasses_hash`,
`callables_protocol_2`, `dataclasses_usage`.

Two upstream sources keep feeding this: `ClassInfo.bases` in
`basilisk-resolver/src/scope/class_types.rs` are raw source strings, and
`visitor/function_info.rs` sets `annotation_text: None` unconditionally — the
direct cause of the `overloads_consistency` false positive.

## Standing rules

1. Fix a failure by rebuilding the verdict on resolved bindings and the
   relation layer — never by restoring text logic, editing a test, or
   suppressing a rule.
2. A failing test that pins real incorrect behaviour outranks a passing
   fixture. Do not chase the count.
3. Conformance figures are a regression indicator only, never a gate, and are
   never published.
