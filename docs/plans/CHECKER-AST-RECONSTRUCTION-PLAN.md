# Rebuild the checker on the AST {#ASTREBUILD}

> **Status (2026-08-08):** the deletion phase is complete and the rebuild has
> not started. Basilisk's former 100% `python/typing` claim is withdrawn, the
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
[`TypingForm`](../../crates/basilisk-resolver/src/canonical/form.rs) — Basilisk's
own enum, whose variants are the **answer** the resolver produces, never the
question a rule asks. The Python spellings that identify each definition site
live in `crates/basilisk-resolver/resources/typing_symbols.toml` as data, and
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

Permitted, because they decide nothing about typing: true builtins (`int`,
`str`, `isinstance`, `object`), dunder names, keyword-argument names at call
sites (`bound=`, `total=`), file extensions, Basilisk's own directive syntax
(`# basilisk:`), and text inside diagnostic **messages**.

**A rebuilt rule that cannot answer its question lawfully emits nothing.** A
silent rule is a tracked gap; a rule that guesses from spelling is a false
conformance claim. Never trade the second for the first.

---

## What is missing {#ASTREBUILD-INVENTORY}

Measured against `HEAD` on 2026-08-06. Every count is reproducible from the
commands given.

### Compile blockers {#ASTREBUILD-INVENTORY-BUILD}

The workspace does not build. `cargo check --workspace` stops in
`basilisk-stubs` before reaching the checker, so the true downstream error count
is unknown until the first item is fixed.

| Site | Break |
|---|---|
| `crates/basilisk-stubs/src/pyi_parser.rs:476,491`, `pyi_parser/syntax.rs:27` | `StubFunction::is_overload` is read and matched but never populated — the decorator recognition that set it was deleted. |
| `crates/basilisk-checker/src/rules/missing_parameter_annotation.rs:191,195` | Calls `shared::decorator_spelled`, which no longer exists. |
| `crates/basilisk-checker/src/rules/calls_argument_count/method_binding.rs:135` | Same. |

### Resolver {#ASTREBUILD-INVENTORY-RESOLVER}

- [`BindingTable`](../../crates/basilisk-resolver/src/canonical/binding.rs) is
  built and exported but **reaches no consumer**. `ResolvedModule` has no
  `bindings` field, so no rule can ask a lawful question today. This is the
  keystone.
- `crates/basilisk-resolver/tests/canonical_registry.rs` does not exist. The 92
  registry entries in `resources/typing_symbols.toml` are unvalidated against
  bundled typeshed.
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
not appear in a `.rs` file at all. The restored ratchet fails on the old file by
construction.

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

Recognition has to be available to `basilisk-stubs`, which parses `.pyi` files
with the same Ruff parser and must answer the same questions. It cannot reach
`canonical/` where that module lives today, because `basilisk-resolver` already
depends on `basilisk-stubs`.

`canonical/` is self-contained — 586 LOC across `form.rs`, `binding.rs`, and
`mod.rs`, importing only `ruff_python_ast`, `serde`, and `std`, with no
`crate::` reference to the rest of the resolver — so it moves without edits.

- [ ] Create `crates/basilisk-canonical` from
      `crates/basilisk-resolver/src/canonical/`, carrying
      `resources/typing_symbols.toml` with it.
- [ ] Re-export `BindingTable`, `TypingForm`, and `CanonicalSymbol` from
      `basilisk-resolver` so no existing consumer's import path changes.
- [ ] Depend on it from `basilisk-stubs` and `basilisk-resolver`. One registry
      answers for the whole workspace; a second copy of the vocabulary anywhere
      is the defect returning.

#### 0b — populate `StubFunction::is_overload` {#ASTREBUILD-PHASE-COMPILE-OVERLOAD}

`pyi_parser.rs:491` routes a function into `self.overloads` on this flag, and
`pyi_parser/syntax.rs:27` no longer sets it.

- [ ] Build a `BindingTable` for each `.pyi` module and set `is_overload` from
      `form_of(decorator)  == Some(TypingForm::Overload)`. A stub that writes
      `from typing import overload as _ov` must group identically.
- [ ] Re-key the receiver decision at `pyi_parser/syntax.rs:20`, which reads
      `decorator.ends_with("staticmethod")` on a rendered string. The builtin
      name is permitted; reconstructing it from rendered text is not.
- [ ] `StubFunction::decorators` is a `Vec<String>` of rendered names — the
      shape that made both defects possible. Carry resolved forms alongside it,
      and stop new decisions being made from the strings.

#### 0c — replace the `decorator_spelled` call sites {#ASTREBUILD-PHASE-COMPILE-DECORATORS}

Three callers reference a helper that no longer exists:
`missing_parameter_annotation.rs:191,195` and
`calls_argument_count/method_binding.rs:135`.

- [ ] Decide `staticmethod` / `classmethod` from the resolved decorator
      **node**. Both are true builtins needing no import, so recognising them is
      permitted — matching them against sliced source text is not.

#### 0d — measure what Phase 0 uncovered {#ASTREBUILD-PHASE-COMPILE-MEASURE}

- [ ] Record the full downstream error list once the workspace compiles. The
      inventory above was taken through a build that stops in `basilisk-stubs`,
      so the real count is unknown and may be larger.
- [ ] `make lint` and `make fmt` clean.

### Phase 1 — deliver binding resolution to consumers {#ASTREBUILD-PHASE-BINDING}

The keystone. Every later phase depends only on this.

- [ ] Add `pub bindings: BindingTable` to `ResolvedModule`, built once in
      `visitor::collect` from the module body.
- [ ] Thread it through `core::collect_from_body` so collectors can consult it
      while building their records.
- [ ] Hang the parsed AST on the analysis context. 28 rules currently call
      `shared::parse_module(module)` and re-parse the file; that is
      O(rules × source) and it is the natural place for the binding table to
      live beside the tree it was derived from.
- [ ] Write `crates/basilisk-resolver/tests/canonical_registry.rs`: every one of
      the 92 registry entries must resolve to a real declaration in bundled
      typeshed. A registry entry naming a symbol typeshed does not declare is a
      **build failure** — that test is what stops the registry becoming a
      spelling table by another route.
- [ ] Test the three cases character matching gets wrong, per module:
      `import X as Y`, `import mod; mod.X`, and a local `class X:` shadow.

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
- [ ] `make mutation-conformance` — the same fixtures with imports aliased and
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
   does not carry), and Basilisk's own rendered stub-signature output.
3. All 43 rules emit again, each with tests that survive import aliasing, dotted
   access, local shadowing, and reformatting.
4. `ResolvedModule` carries the binding table; `canonical_registry.rs` passes.
5. `InferredType` no longer identifies a type by a string.
6. The three gates in [ASTREBUILD-PHASE-EVIDENCE](#ASTREBUILD-PHASE-EVIDENCE)
   agree, and the published figure is whatever they produce.
