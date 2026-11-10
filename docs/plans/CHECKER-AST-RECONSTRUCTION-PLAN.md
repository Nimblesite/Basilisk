# Rebuild the checker on the AST {#ASTREBUILD}

> **Status (2026-08-08):** the deletion phase is complete and the rebuild has
> begun: [Phase 0a](#ASTREBUILD-PHASE-COMPILE-CANONICAL),
> [0b](#ASTREBUILD-PHASE-COMPILE-OVERLOAD), and the binding-table threading of
> [0d](#ASTREBUILD-PHASE-COMPILE-MEASURE) are done — `basilisk-canonical`,
> `basilisk-resolver`, and `basilisk-stubs` compile clean under workspace lints
> and the build now stops in `basilisk-checker`. The registry-load defect that
> silently emptied the `TypingForm` index is fixed and pinned by
> `crates/basilisk-canonical/tests/canonical_registry.rs`. The binding table is
> now scope- and order-correct (position-aware events, module frame only,
> star-import materialisation, builtin fallback), pinned by
> `crates/basilisk-canonical/tests/binding_table.rs`; the [0c](#ASTREBUILD-PHASE-COMPILE-DECORATORS)
> resolver mechanism landed, so the function-binary failure in the 161 below
> has since been fixed and its pin passes. Deleted resolver
> collectors are stubbed to empty vectors (inert, never satisfied), pinned by
> **161 failing resolver tests** (462 pass; measured per-binary on 2026-08-08:
> annotation 14, class 8, function 1, misc 11, mutant 31, protocol 25,
> resolution 1, type_system 36, typeddict 34) —
> [Phase 2](#ASTREBUILD-PHASE-RESOLVER) owns them all, and none may be deleted
> or weakened.
> Basilisk's former 100% `python/typing` claim is withdrawn, the
> project is not listed in the [official results](https://github.com/python/typing/blob/main/conformance/results/results.html),
> and the current conformance level is **unknown**. Nothing in this plan
> licenses publishing a number; [ASTREBUILD-PHASE-EVIDENCE](#ASTREBUILD-PHASE-EVIDENCE)
> is the only route to one.

Companion documents:

- [CONFORMANCE-SPELLING-CHEAT-INVENTORY.md](../CONFORMANCE-SPELLING-CHEAT-INVENTORY.md) —
  what was deleted and why.
- [CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md](CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md) —
  the deletion plan this one succeeds.
- [CHKARCH-RECOGNITION](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-RECOGNITION) —
  the normative rule this plan implements.

---

## The law every line of this rebuild obeys {#ASTREBUILD-LAW}

Recognition is a question about **definitions**, answered from the AST plus
binding resolution. It is never a question about the characters at the use site.

A rebuilt rule asks *what does this expression refer to?* and receives a
[`TypingForm`](../../crates/basilisk-canonical/src/form.rs) — Basilisk's
own enum, whose variants are the **answer** the resolver produces, never the
question a rule asks. The Python spellings that identify each definition site
live in `crates/basilisk-canonical/resources/typing_symbols.toml` as data, and
appear in no Rust file.

Banned in every crate, in every form — see the symbol-naming ban in
[`CLAUDE.md`](../../CLAUDE.md):

| Banned | Why |
|---|---|
| `name.id.as_str() == "TypeVar"` | `import TypeVar as TV` is still a `TypeVar`; a local `class TypeVar:` is not. |
| `matches!(base, "Protocol" \| "Generic")` | A match arm is a comparison with extra syntax. |
| `denotes(resolver, expr, "ClassVar")` | Passing the name as an argument is the same cheat behind an API. |
| `const FORMS: &[&str] = &["Final", …]` | A table of spellings is a comparison hoisted into a static. |
| `source.lines()` + `starts_with("def ")` | Re-lexing text the parser already parsed, badly. |
| `slice_span(&module.source, span).starts_with("tuple[")` | Same defect one level down, on rendered annotation text. |

Permitted, because they decide nothing about typing: builtin spellings inside
`typing_symbols.toml` and the registry's builtin fallback ONLY (a use of
`int`/`isinstance`/`object` resolves through `form_of_with_builtins` — Python
lets every name be shadowed, rebound, or aliased, so no use site may be
compared against a builtin spelling), dunder names at definition sites,
keyword-argument names at call sites (`bound=`, `total=`), file extensions,
Basilisk's own directive syntax (`# basilisk:`), and text inside diagnostic
**messages**.

**A rebuilt rule that cannot answer its question lawfully emits nothing.** A
silent rule is a tracked gap; a rule that guesses from spelling is a false
conformance claim. Never trade the second for the first.

---

## What is missing {#ASTREBUILD-INVENTORY}

Measured against `HEAD` on 2026-08-06. Every count is reproducible from the
commands given.

### Compile blockers {#ASTREBUILD-INVENTORY-BUILD}

The workspace does not build. As first measured, `cargo check --workspace`
stopped in `basilisk-stubs`; [Phase 0a/0b](#ASTREBUILD-PHASE-COMPILE) fixed
that, and the build now stops in `basilisk-resolver` —
[0d](#ASTREBUILD-PHASE-COMPILE-MEASURE) classifies its 32 errors. The checker
and LSP have still never been reached, so their error counts remain unknown.

| Site | Break | Status |
|---|---|---|
| `basilisk-stubs` `pyi_parser` | `StubFunction::is_overload` was read and matched but never populated — the decorator recognition that set it was deleted. | Fixed in [0b](#ASTREBUILD-PHASE-COMPILE-OVERLOAD) |
| `crates/basilisk-checker/src/rules/missing_parameter_annotation.rs:191,195` | Calls `shared::decorator_spelled`, which no longer exists. | Open — [0c](#ASTREBUILD-PHASE-COMPILE-DECORATORS) |
| `crates/basilisk-checker/src/rules/calls_argument_count/method_binding.rs:135` | Same. | Open — [0c](#ASTREBUILD-PHASE-COMPILE-DECORATORS) |

### Resolver {#ASTREBUILD-INVENTORY-RESOLVER}

- [`BindingTable`](../../crates/basilisk-canonical/src/binding.rs) is
  built and exported but **reaches no rule**. `ResolvedModule` has no
  `bindings` field, so no rule can ask a lawful question today. This is the
  keystone. (The stub parser now builds its own table per `.pyi` module —
  [0b](#ASTREBUILD-PHASE-COMPILE-OVERLOAD) — which is correct: stubs are
  separate modules with separate imports.)
- `crates/basilisk-canonical/tests/canonical_registry.rs` now pins the load
  contract: every `[[symbol]]` entry in `resources/typing_symbols.toml` must
  resolve through the live registry to a `TypingForm` (a parse failure was
  silently degrading to an empty index, deadening every `form_of` in the
  workspace). Validation of the entries **against bundled typeshed** is still
  missing — that half stays a [Phase 1](#ASTREBUILD-PHASE-BINDING) item.
- `visitor/protocol.rs` is an empty module. `visitor/enum_checks.rs` retains
  only an unrelated builtin-attribute constant.
- `visitor/protocol_ext.rs` keeps `base_type_name` / `unqualified_base`, which
  split rendered annotation **text** on `[` and `.`.
- `visitor/class_info_ext.rs` no longer populates the `ClassInfo` flags that
  described a class's declared nature.

### Checker rules {#ASTREBUILD-INVENTORY-RULES}

**43 registered rules emit nothing.** Reproduce with:

```bash
cd crates/basilisk-checker/src/rules
for f in $(grep -rl '_module: &ResolvedModule' . --include='*.rs'); do
  grep -q '_diagnostics: &mut Vec<Diagnostic>' "$f" && echo "$f"
done
```

That query returns 49 files; six are **not** part of this plan and must stay as
they are — `unused_dependency.rs` and `stale_lock_file.rs` are pre-existing
skeletons awaiting workspace-level data, and the four `suppression_*.rs` rules
are emitted by the suppression engine rather than from their own `check`.

The remaining 43 are grouped below by the `TypingForm` each one needs, because
that grouping *is* the implementation order — every rule in a group unblocks
together once its form resolves.

| Group | Forms needed | Rules |
|---|---|---|
| Annotation qualifiers | `ClassVar`, `FinalQualifier`, `Annotated`, `Required`, `NotRequired`, `ReadOnly` | `classes_classvar`, `qualifiers_final_annotation`, `qualifiers_annotated`, `qualifiers_annotated_2`, `typeddicts_required` |
| Protocols | `Protocol`, `RuntimeCheckable` | `protocols_definition`, `protocols_definition_2`, `protocols_explicit_2`, `protocols_explicit_3`, `protocols_generic`, `protocols_merging`, `protocols_modules`, `protocols_runtime_checkable_2`, `protocols_subtyping`, `protocols_variance_2`, `protocols_class_objects_2` |
| Type parameters | `TypeVar`, `TypeVarTuple`, `ParamSpec`, `Generic` | `generics_variance_inference`, `generics_defaults_referential_2`, `generics_upper_bound_2`, `generics_typevartuple_basic_3`, `generics_typevartuple_callable` |
| `Self` | `SelfType` | `generics_self_basic`, `generics_self_attributes`, `generics_self_usage` |
| Named tuples | `NamedTuple`, `CollectionsNamedTuple` | `namedtuples_define_class`, `namedtuples_type_compat` |
| Enumerations | `EnumBase` family, `EnumMember`, `EnumNonmember` | `enums_expansion`, `enums_members_2` |
| Literals | `Literal`, `LiteralString` | `literals_literalstring`, `literals_semantics_2`, `literals_parameterizations` |
| Bottom types | `Never`, `NoReturn` | `specialtypes_never`, `specialtypes_never_2` |
| Dataclasses | `Dataclass`, `DataclassTransform`, `DataclassField` | `dataclasses_order`, `dataclasses_transform_class` |
| Method decorators | `Overload`, `Override`, `FinalDecorator` | `classes_override`, `classes_override_3`, `constructors_callable` |
| Callables | `Callable`, `Unpack`, `TypedDict` | `callables_kwargs`, `callables_subtyping` |
| Narrowing guards | `TypeIs`, `TypeGuard` | `narrowing_typeis` |
| Generators | `Generator` family | `annotations_generators_2` |
| Version gates | `TypeCheckingFlag` | `directives_version_platform` |

Two further rules are **partially** live and keep their resolver-derived half:
`dataclasses_slots` lost `self.`-attribute discovery and `__slots__` access
detection; `generics_type_erasure` lost its class-attribute scanner.

### The annotation-text layer {#ASTREBUILD-INVENTORY-TEXT}

`InferredType::Named(String)` makes the checker's type representation a string,
so ~130 sites still decide meaning by matching rendered annotation text:

```bash
grep -rn "slice_span"      crates/basilisk-checker/src --include='*.rs' | wc -l   # 94
grep -rn "from_annotation" crates/basilisk-checker/src --include='*.rs' | wc -l   # 36
```

Every name involved is a true builtin, so the symbol-naming ban permits the
names — but the *mechanism* is string matching on code, and it is why
`starts_with("tuple[")` and `split(" | ")` still appear. This layer is already
condemned by
[TYPEINF-LEGACY](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-LEGACY);
[ASTREBUILD-PHASE-TYPEEXPR](#ASTREBUILD-PHASE-TYPEEXPR) is its demolition.

### LSP {#ASTREBUILD-INVENTORY-LSP}

Deleted because keyword scanning **was** the whole mechanism, not a detail of it:

| Feature | Spec | Residue |
|---|---|---|
| Move symbol (new + existing file) | [REFACTOR-MOVE](../specs/LSP-REFACTORING-SPEC.md#REFACTOR-MOVE) | `MOVE_SYMBOL` is still advertised in `server/commands.rs:109` with no code action producing it. |
| Extract function | [REFACTOR-EXTRACT-FUNC](../specs/LSP-REFACTORING-SPEC.md#REFACTOR-EXTRACT-FUNC) | — |
| Extract constant | [REFACTOR-EXTRACT-VAR](../specs/LSP-REFACTORING-SPEC.md#REFACTOR-EXTRACT-VAR) | `extract_variable` survives. |
| Add `__all__` | [LSPFMT-IMPORTS](../specs/LSP-FORMATTING-SPEC.md#LSPFMT-IMPORTS) | — |
| Auto-import insertion point | [LSPARCH-FEATURES-CODEACTIONS](../specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CODEACTIONS) | `server/handlers/features.rs` hardcodes `Position::new(0, 0)`. |
| `self.attr` / `cls.attr` rename sweep | [REFACTOR-RENAME-SCOPE](../specs/LSP-REFACTORING-SPEC.md#REFACTOR-RENAME-SCOPE) | Identifier, keyword-arg, and docstring sweeps survive. |
| Import rewriting on file rename | [REFACTOR-RENAMEMOD](../specs/LSP-REFACTORING-SPEC.md#REFACTOR-RENAMEMOD) | `collect_import_edits_for_rename` is inert; the path→module helpers survive. |

---

## Salvage: what survives, what returns, what stays deleted {#ASTREBUILD-SALVAGE}

Scanned exhaustively, not sampled. All 216 `.rs` files ever deleted anywhere in
history were recovered at their parent commit and scored three ways: AST signal
(`Expr::`, `Stmt::`, `Visitor`, span access), text-mechanism signal
(`starts_with("`, `slice_span`, `.lines()`, `split(`), and — the decisive one —
**Python typing-symbol string literals**, which catch the
`denotes(resolver, expr, "ClassVar")` cheat that signal-counting alone misses.
Deleted hunks in files that survived were scored the same way.

**No deleted file is restorable as-is.** Every candidate splits along the same
seam, and it is worth stating precisely because it recurs in Phase 4:

| Half | State | Disposition |
|---|---|---|
| Traversal and structure — visitors, depth tracking, node matching, span extraction | Lawful AST work | Read as reference; re-derive |
| Recognition — which symbol is this? | `denotes_form(resolver, expr, "ClassVar")` | The banned mechanism; never returns |
| Type comparison — are these compatible? | `ann_str`, `slice_span`, rendered-text matching | The condemned layer; Phase 5 |

Three classes of apparent loss turned out not to be losses at all, and are
recorded so nobody re-investigates them: `names_unbound.rs` (594 LOC) was
**split** into `names_unbound/{mod,bindings,scan}.rs` and is 635 LOC today;
`e0145`/`e0148` predate the error-code rename; and `visitor.rs` (7,730 LOC,
March 2026) became the `visitor/` directory.

### Reference material, not restorable: the gutted rule bodies {#ASTREBUILD-SALVAGE-RULES}

`4c2f124b` gutted two rules far below what the inventory's "emits nothing"
count conveys — `classes_classvar` went from 737 LOC across five files to 33,
and `dataclasses_transform_class` from 855 to 37. Both are on the
[Phase 4 list](#ASTREBUILD-INVENTORY-RULES), and what they lost is spec
semantics that took real work to encode:

- `classes_classvar/instance.rs` (193 LOC, **zero** symbol literals) walked
  method bodies with a proper `Visitor`, tracking class and method depth, to
  find `self.x: ClassVar[T]` — invalid per PEP 526 — and instance assignments
  to class-level `ClassVar`. Its own header records that it was written *to
  replace* a byte scanner that "could not tell code from a docstring and
  hardcoded the fixture's `CV` import alias". The traversal is exemplary. It
  calls `helpers::is_classvar`, which is `denotes_form(resolver, expr,
  "ClassVar")` — so the file dies on its recognition call, not its structure.
- `dataclasses_transform_class/converter.rs` (475 LOC, zero symbol literals)
  implemented PEP 681 `converter=`: the synthesized `__init__` accepts the
  converter's first parameter type, not the field's declared type, across
  defaults, constructor arguments, and attribute assignment. The rule is
  correct and hard-won; it compares types through `ann_str` and `slice_span`,
  so it cannot return before [Phase 5](#ASTREBUILD-PHASE-TYPEEXPR).

- [ ] When Phase 4 reaches these two rules, read the deleted bodies first at
      `4c2f124b^` for the spec cases they enumerate, then derive the rule from
      the typing specification as [Phase 4](#ASTREBUILD-PHASE-RULES) requires.
      They are a checklist of edge cases, never a source to copy.

### Rebuild from, do not restore: the resolver collectors {#ASTREBUILD-SALVAGE-COLLECTORS}

The 17 collectors `visitor/mod.rs` still calls were vetted individually against
their last living version (`01956fbb^`, `7ca57287^`). Sixteen decide from
spellings — `collect_typeddict_calls`, `collect_newtype_calls`,
`extract_generic_params`, `parse_dataclass_transform_decorator`,
`collect_protocol_self_violations` and the rest each compare rendered names or
match hardcoded symbol text. None returns.

One's **traversal** is clean — and only the traversal. `collect_typevar_calls`
has a statement walk (assignments, recursion into class bodies) worth keeping.
Everything around that loop that decides what a callee or a name *means* — the
spelling recognizer, the string `callee` parameter it threads through, the
uppercase-letter name guess — is the banned mechanism and stays dead.

- [ ] Rebuild it keeping the walk and the argument-shape reading; resolve the
      callee through `form_of`; carry the resolved form rather than a string;
      treat a name as a type variable only when its declaration was collected.

### Rebuild from, do not restore: the type-expression evaluator {#ASTREBUILD-SALVAGE-FORMS}

`crates/basilisk-checker/src/annotation/forms.rs` lost 142 lines in `31ef02d8`
— the only deleted file with real AST signal and no text signal. It evaluated
subscripted special forms from their **argument expressions**: `Callable[[P], R]`
including the `[]` and `...` cases, `Concatenate[X, P]` prefix extraction,
`Generator`, `Literal`, `Optional`/`Union`, and the PEP 647/742 guard payloads.
That is genuine type-expression evaluation over `Expr` nodes and it is worth
having back.

It is not restorable as written. It dispatched on a **lowercased resolved member
name** (`"literal"`, `"callable"`, `"concatenate"`), which the older cheat
inventory classed as legitimate because the string came from the import
cascade rather than the use site. [ASTREBUILD-LAW](#ASTREBUILD-LAW) is stricter
and supersedes it: the answer must be a `TypingForm`, and a Python spelling may
not appear in a `.rs` file at all.

- [ ] Rebuild the evaluator in [Phase 5](#ASTREBUILD-PHASE-TYPEEXPR) with the
      argument-walking structure intact and the dispatch re-keyed from
      lowercased strings to `BindingTable::form_of`.

### Already in the tree, and clean: the inference engine {#ASTREBUILD-SALVAGE-INFERENCE}

**The type-inference system is not among the losses.** It survives whole, it is
larger than when it was written, and it is very nearly free of the defect this
plan exists to remove. This is the single most important fact for sequencing
the rebuild, because it means Phase 5 has a working engine to swap *into*.

Its history reads as a loss and is not one. `e3e97d30` ([#377](https://github.com/Nimblesite/Basilisk/pull/377)) built the shared
inference, narrowing, and subtyping foundations. `3c328130` ([#413](https://github.com/Nimblesite/Basilisk/pull/413)) then appears
to gut it — `inference.rs` drops from 670 lines to 130, `rules/shared.rs` from
628 to 307. Both are **consolidations**: that commit created `expr_type.rs`
(+359) and exploded `tyeval.rs` into `tyeval/{lower,eval,accept,term,queries}`
(~1,390 lines), while `shared.rs` became `shared/{oracle,judge,class_walks,
module_types,returns_judge}`. Today the machinery totals roughly 8,200 lines:

| Component | LOC |
|---|---|
| `bidir/` — bidirectional engine, constraints, solving, tyvars | 2,966 |
| `narrow/` — flow, guards, reachability, rebinding, set ops | 2,253 |
| `tyeval/` — lowering, evaluation, acceptance, terms | 1,374 |
| `types.rs`, `expr_type.rs`, `subtyping.rs`, `inference.rs` | 1,319 |

Audited with both scans — text mechanisms (`slice_span`, `from_annotation`,
`ann_str`, `.lines()`, `starts_with("`) and Python typing-symbol string
literals, over production code with comments stripped:

- **Zero** unlawful symbol recognition. The only symbol literals in the whole
  system are `write!(f, "Any")` and `write!(f, "Never")` inside
  `impl fmt::Display for InferredType` — rendering, which
  [the law](#ASTREBUILD-LAW) permits.
- **Nine** text-mechanism sites, and every one is the *same* boundary:
  `InferredType::from_annotation` (six calls, in `tyeval/lower.rs`,
  `narrow/flow.rs`, `narrow/guards.rs`) and three
  `name == "type" || name.starts_with("type[")` tests that exist only because
  the name is a `String`. `type` is a true builtin, so the *name* is permitted;
  the *mechanism* is not.

The algorithms — bidirectional checking, constraint solving, type-term
lowering and acceptance, flow-sensitive narrowing, reachability, subtyping —
never touch source text. `InferredType::Named(String)` is the sole leak, and it
is a boundary rather than a diffusion.

- [ ] Sequence [Phase 5](#ASTREBUILD-PHASE-TYPEEXPR) accordingly. Its 130-site
      count lives in the **rules**, not the engine; the engine needs one
      representation change at nine call sites. Do the engine's boundary first
      and the rules' call sites become mechanical.

## The pattern every rebuild follows {#ASTREBUILD-PATTERN}

One shape, used everywhere, so review is mechanical.

```rust
use basilisk_resolver::{BindingTable, TypingForm};
use ruff_python_ast::Expr;

/// PEP 591: `Final[T]` may not nest inside another qualifier.
fn final_element<'a>(bindings: &BindingTable, annotation: &'a Expr) -> Option<&'a Expr> {
    // The question is an expression. The answer is a form. No spelling is
    // written, passed, or compared anywhere in this function.
    bindings.subscript_element(annotation, TypingForm::FinalQualifier)
}
```

Three properties make this lawful, and a rebuilt rule must have all three:

1. **The question is a node**, never a string. `form_of(expr)`, not
   `form_of("Final")`.
2. **The answer is a `TypingForm`**, a Basilisk enum. Comparing against
   `TypingForm::FinalQualifier` compares against our own vocabulary, not
   Python's.
3. **Adding a spelling means editing `typing_symbols.toml`**, never a `.rs`
   file. If a rebuild tempts you to type a Python symbol name into Rust, the
   registry entry is missing — add it there.

Reviewer's test: if `from typing import Final as F` breaks the rule, the rebuild
is wrong. So does a module that defines its own `class Final:`.

---

## Phases {#ASTREBUILD-PHASES}

Strictly ordered. Each phase is independently mergeable and leaves the tree
greener than it found it.

### Phase 0 — restore the build {#ASTREBUILD-PHASE-COMPILE}

Nothing else can be measured until `cargo check --workspace --all-targets`
completes. Three breaks, and one crate move they depend on.

#### 0a — put the canonical layer where every crate can reach it {#ASTREBUILD-PHASE-COMPILE-CANONICAL}

**Done.** Recognition has to be available to `basilisk-stubs`, which parses
`.pyi` files with the same Ruff parser and must answer the same questions. It
could not reach `canonical/` inside the resolver, because `basilisk-resolver`
already depends on `basilisk-stubs`.

- [x] `crates/basilisk-canonical` created by `git mv` from
      `crates/basilisk-resolver/src/canonical/`, carrying
      `resources/typing_symbols.toml` with it. Dependencies: `ruff_python_ast`,
      `serde`, `toml` — nothing else.
- [x] `BindingTable`, `TypingForm`, and `CanonicalSymbol` re-exported from
      `basilisk-resolver` (`src/lib.rs`) so no existing consumer's import path
      changes.
- [x] `basilisk-stubs` and `basilisk-resolver` both depend on it. One registry
      answers for the whole workspace; a second copy of the vocabulary anywhere
      is the defect returning.

The move surfaced that the module had never compiled under workspace lints
(the build always died in `basilisk-stubs` first): 89 undocumented `TypingForm`
variants and 7 `unused_results` violations, all fixed in place.

#### 0b — populate `StubFunction::is_overload` {#ASTREBUILD-PHASE-COMPILE-OVERLOAD}

`pyi_parser.rs` routed functions into `self.overloads` on a flag nothing set —
the decorator recognition that set it was deleted.

- [x] `StubExtractor` builds one `BindingTable` per `.pyi` module
      (`Arc`-shared into `if`-branch clones) and sets `is_overload` from
      `has_decorator_form(bindings, decorators, TypingForm::Overload)`. A stub
      that writes `from typing import overload as _ov` groups identically; a
      stub that defines its own `overload` does not resolve at all.
- [x] The receiver decision, which read `decorator.ends_with("staticmethod")`
      on a rendered string, is re-keyed to `has_staticmethod_decorator`: a bare
      `Name` decorator node carrying the builtin identifier, rejected whenever
      the module binds that name itself (`BindingTable::binds_name`). The
      builtin is **fixed inside the function** — an API taking the name as a
      parameter would be the third banned row of
      [the law](#ASTREBUILD-LAW) behind a helper.
- [ ] `StubFunction::decorators` is still a `Vec<String>` of rendered names —
      the shape that made both defects possible. 0b added a lawful path for the
      stub parser's own decisions; it did **not** retire the strings. Live
      downstream consumers that still decide from them:
      `shared::overload_decorated(resolver, &f.decorators)` in the six
      `overloads_*` rules, and
      `overloads_consistency_3.rs::is_type_only_decorator`, which `rsplit`s the
      rendered name on `.` and matches
      `"staticmethod" | "classmethod" | "property"` — the banned match-arm
      form. Carry resolved forms on `StubFunction`, migrate those consumers,
      then delete the string field.

#### 0c — replace the `decorator_spelled` call sites {#ASTREBUILD-PHASE-COMPILE-DECORATORS}

Three callers reference a helper that no longer exists:
`missing_parameter_annotation.rs:191,195` and
`calls_argument_count/method_binding.rs:135`.

- [x] Resolver mechanism landed: `TypingForm::StaticMethod` / `ClassMethod`
      registry entries under `modules = ["builtins"]` (validated against
      bundled typeshed), `BindingTable::form_of_with_builtins` for the
      no-import fallback, and `FunctionInfo::{is_staticmethod,is_classmethod}`
      populated from the resolved decorator **node** (`is_abstractmethod` via
      `abc.abstractmethod` landed alongside). Pinned by
      `tests/resolver/test_decorators.rs::builtin_decorator_flags_do_not_depend_on_spelling`
      and `tests/binding_table.rs::builtin_fallback_respects_module_rebinds`.
- [ ] Swap the three call sites (`missing_parameter_annotation.rs:191,195`,
      `calls_argument_count/method_binding.rs:135`) onto those flags. They were
      deliberately left referencing the deleted helper rather than converted
      into invisible no-ops; the swap is now mechanical.

#### 0d — measured: what Phase 0 uncovered {#ASTREBUILD-PHASE-COMPILE-MEASURE}

Done. `basilisk-canonical` and `basilisk-stubs` compile, so the build now
reaches `basilisk-resolver` and stops there with **32 errors**. Nothing
downstream of the resolver has been compiled yet, so the checker and LSP counts
remain unknown.

**Phase 0 cannot finish inside Phase 0.** The resolver does not fail on
diagnostics it can no longer emit; it fails to *compile*, because the deleted
collectors are load-bearing for `visitor::collect`. Restoring the build
therefore requires the front half of Phase 1 and most of Phase 2:

| Cause | Count | Phase that owns it |
|---|---|---|
| Deleted collectors and helpers still referenced (`E0425`/`E0432`) | 17 | [Phase 2](#ASTREBUILD-PHASE-RESOLVER) |
| Callers not passing `&BindingTable` to already-migrated predicates | 7 | [Phase 1](#ASTREBUILD-PHASE-BINDING) |
| Signature drift in `typeddict`/`core`/`final_readonly` helpers | 6 | [Phase 2](#ASTREBUILD-PHASE-RESOLVER) |
| `ClassInfo` initializer missing 13 declared-nature fields | 1 | [Phase 3](#ASTREBUILD-PHASE-CLASSINFO) |
| Unused import left behind by the deletions | 1 | [Phase 2](#ASTREBUILD-PHASE-RESOLVER) |

**Phase 1 is already part-built**, which the inventory did not record: 13 leaf
predicates across `visitor/{annotations,class_info,dataclass,final_readonly}.rs`
already take `&BindingTable` and decide through `form_of`. They are correct and
they are unreachable — nothing constructs a table or passes one. Threading it is
what makes them live, and it is the cheapest way to shrink the error count.

- [x] Thread `&BindingTable` from `visitor::collect` to the 13 migrated
      predicates, closing the 7 arity errors. `visitor::collect` builds one
      table and passes it through `core::collect_from_body` (via
      `CollectSinks`), the typevar collector, and the `Final` violation walk;
      the resolver compiles clean under workspace lints.
- [ ] Rebuild the 17 collectors ([Phase 2](#ASTREBUILD-PHASE-RESOLVER)); only
      `collect_typevar_calls` is recoverable from history, and only its
      traversal — see [ASTREBUILD-SALVAGE-COLLECTORS](#ASTREBUILD-SALVAGE-COLLECTORS).
- [ ] `make lint` and `make fmt` clean.

#### 0e — a spelling heuristic found and deleted {#ASTREBUILD-PHASE-COMPILE-DELETION}

`visitor/typevar.rs::is_typevar_like_name` decided a name was a type parameter
because it was **a single uppercase letter**, and its own doc comment conceded
the guess: "almost universally `TypeVars`". `bound_refs_outer_typeparam` used it
to treat any such name as an outer type parameter, so a module-level
`class T:` was read as a type parameter and a genuine type parameter named `Key`
was invisible.

Deleted under the [CLAUDE.md](../../CLAUDE.md) protocol, with two failing tests
left behind in `tests/resolver/test_pep695.rs` pinning both directions. The
explicit `outer_typeparams` membership test remains and is lawful.

- [x] Make those two tests pass by resolving the bound expression through the
      module's bindings, not by restoring a name-shape guess. `typevar.rs`
      resolves factory callees via `factory_form(bindings, …)` and decides
      outer-parameter references by explicit `outer_typeparams` membership;
      both directions pass
      (`pep695_bound_referencing_an_outer_multiletter_typeparam_is_a_violation`,
      `pep695_bound_naming_a_real_class_is_not_an_outer_typeparam`).

### Phase 1 — deliver binding resolution to consumers {#ASTREBUILD-PHASE-BINDING}

The keystone. Every later phase depends only on this.

- [x] Add `pub bindings` to `ResolvedModule`, built once in `visitor::collect`
      from the module body. Carried as `ModuleBindings`, a deref-transparent
      wrapper giving the table the same equality treatment as `LazyAst` (a
      pure function of `source` carries no independent identity — and the
      public `BindingTable` itself must not grow a lying `PartialEq`).
- [x] Thread it through `core::collect_from_body` so collectors can consult it
      while building their records. Done as part of 0d: `&BindingTable` is the
      first parameter of every collection walk in `visitor/core.rs`.
- [ ] Hang the parsed AST on the analysis context. 28 rules currently call
      `shared::parse_module(module)` and re-parse the file; that is
      O(rules × source) and it is the natural place for the binding table to
      live beside the tree it was derived from.
- [x] Registry-vs-typeshed validation: every registry entry must resolve to a
      real declaration in bundled typeshed. Landed as
      `crates/basilisk-stubs/tests/registry_typeshed_validation.rs` — the stubs
      crate is where typeshed is bundled, so the test lives there rather than
      at the resolver path first proposed. A registry entry naming a symbol
      typeshed does not declare is a **build failure** — that test is what
      stops the registry becoming a spelling table by another route.
- [x] Test the three cases character matching gets wrong, per module:
      `import X as Y`, `import mod; mod.X`, and a local `class X:` shadow —
      `crates/basilisk-canonical/tests/binding_table.rs`
      (`alias_dotted_and_shadow_still_resolve`), alongside pins for scope
      containment, positional rebinding, guarded imports, compound-target
      rebinds, star-import materialisation, and the builtin fallback.

### Phase 2 — rebuild resolver collectors {#ASTREBUILD-PHASE-RESOLVER}

- [ ] Rebuild the ~13 deleted collectors on `form_of`: type-parameter factory
      calls, functional `TypedDict`/`NamedTuple`/`NewType` calls, `TypeAliasType`
      construction, protocol bases, and enum member classification.
- [ ] Delete `protocol_ext::base_type_name` and `unqualified_base`; base classes
      are `Expr` nodes with exact spans, and splitting their rendered text on
      `[` and `.` is the defect this plan exists to remove.
- [ ] Each collector lands with tests for alias, dotted, and shadowed forms.

### Phase 3 — rebuild `ClassInfo` {#ASTREBUILD-PHASE-CLASSINFO}

- [ ] Repopulate the declared-nature flags in `visitor/class_info_ext.rs` from
      resolved bases and decorators.
- [ ] Rebuild `extract_generic_params` from `StmtClassDef::type_params` (PEP 695)
      and from resolved `Generic[...]`/`Protocol[...]` bases (PEP 484).
- [ ] Rebuild `parse_dataclass_transform_decorator` from the decorator's resolved
      form; its keyword-argument **names** (`eq_default=`, `kw_only_default=`,
      `field_specifiers=`) are permitted, as they need no import.

### Phase 4 — rebuild the 43 rules {#ASTREBUILD-PHASE-RULES}

Take the [inventory groups](#ASTREBUILD-INVENTORY-RULES) in the order listed:
qualifiers first (smallest, exercises the pattern end to end), protocols next
(largest single cluster and the most spec surface), then the rest as their forms
land.

Per rule, in this order:

- [ ] Read the [typing specification](https://typing.python.org/en/latest/spec/index.html)
      section the rule implements. The rule is derived from the specification,
      not from any fixture, and not from Pyright or Pyrefly source.
- [ ] Write failing tests **first**, including the alias/dotted/shadow trio and
      at least one case with no analogue in the conformance suite.
- [ ] Implement using [the pattern](#ASTREBUILD-PATTERN).
- [ ] Confirm the rule stays registered in `all_rules()` and that its
      `ErrorCode` and `docs_url` are unchanged.
- [ ] Re-run `python3 scripts/gen_rules_reference.py --data` if the rule's
      doc-comment body changed ([WEBSITE-ERROR-PAGES-DRIFT](../specs/WEBSITE-ERROR-PAGES-SPEC.md)).

Do not batch. One rule per commit keeps the false-positive source obvious when
the fixture regression moves.

### Phase 5 — demolish the annotation-text layer {#ASTREBUILD-PHASE-TYPEEXPR}

The largest and last structural item; it is why `InferredType::Named(String)`
exists at all.

- [ ] Replace `InferredType::Named(String)` with a resolved reference to a
      declaration, so a type is identified by what it *is* rather than by how it
      was spelled.
- [ ] Delete `types_parsing::from_annotation` and its 36 call sites; annotations
      become type expressions evaluated through
      [TYPEINF-ANNOTATION-RESOLUTION](../specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION).
- [ ] Delete the 94 `slice_span` decision sites. `slice_span` may survive only
      for diagnostic **message** text, never for a verdict.
- [ ] Convert this phase's checklist into per-rule issues as the count falls;
      130 sites is too coarse to review as one change.

### Phase 6 — rebuild the LSP refactorings {#ASTREBUILD-PHASE-LSP}

- [ ] Auto-import insertion point, from the resolved import block's AST range.
      Currently hardcoded to line 0 and user-visible; fix first.
- [ ] Import rewriting on file rename, from `ImportInfo` spans.
- [ ] `self.attr` / `cls.attr` rename, from attribute-access nodes whose value
      resolves to the receiver parameter.
- [ ] Extract function, over AST statement ranges with real data-flow analysis
      per [REFACTOR-EXTRACT-FUNC-ALGO](../specs/LSP-REFACTORING-SPEC.md#REFACTOR-EXTRACT-FUNC-ALGO).
- [ ] Move symbol, over definition nodes and the import graph. Until it lands,
      `MOVE_SYMBOL` is advertised with no producer — either rebuild it or
      withdraw the command; do not leave it half-registered
      ([LSPARCH-CMDREG](../specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CMDREG)).
- [ ] Extract constant and add `__all__`, over module-level AST.

### Phase 7 — re-establish the evidence {#ASTREBUILD-PHASE-EVIDENCE}

No number is publishable until every box here is ticked.

- [ ] `python3 conformance/run_conformance.py` — pristine fixture regression,
      freshly cloned, unmodified upstream scorer, release binary from this
      checkout, default config.
- [ ] `make conformance MUTATED=1` — the same fixtures with imports aliased and
      formatting varied. **A verdict that changes under a semantics-preserving
      rename means the rebuild reproduced the defect.**
- [ ] Off-suite tests derived independently from the specification, for every
      rebuilt rule.
- [ ] `make test` — coverage ratchet up, mutation ratchet up, no test deleted,
      weakened, or ignored.
- [ ] Publish the honest number **even if it is lower** than the withdrawn 100%,
      with the method, robustness checks, and limitations stated.
- [ ] Only then consider a new submission to `python/typing`, and describe
      Basilisk as listed only after that submission is independently validated
      and accepted.

---

## Acceptance {#ASTREBUILD-ACCEPTANCE}

1. No Rust file in any crate names a Python typing symbol to decide what an
   expression means.
2. No production code reconstructs Python structure from raw source text.
   Permitted exceptions, and no others: line **geometry** for diagnostic spans,
   Basilisk's own `# basilisk:` directives (genuinely comments, which the AST
   does not carry), and Basilisk's own rendered stub-signature output **for
   presentation only** — rendered text may never feed a typing verdict
   ([CHKARCH-RECOGNITION-PERMITTED](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-RECOGNITION-PERMITTED)).
3. All 43 rules emit again, each with tests that survive import aliasing, dotted
   access, local shadowing, and reformatting.
4. `ResolvedModule` carries the binding table; `canonical_registry.rs` passes.
5. `InferredType` no longer identifies a type by a string.
6. The three gates in [ASTREBUILD-PHASE-EVIDENCE](#ASTREBUILD-PHASE-EVIDENCE)
   agree, and the published figure is whatever they produce.
