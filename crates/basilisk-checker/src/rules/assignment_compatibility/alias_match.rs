// ############################################################################
// # BROKEN — THIS FILE DOES NOT COMPILE. DO NOT "FIX" IT BY RESTORING TEXT   #
// # MATCHING.                                                                #
// #                                                                          #
// # Deleted helper this file called:                                         #
// #   InferredType::from_annotation (types_parsing.rs)
// #                                                                          #
// # That helper decided types from the SPELLING of source text (lowercased   #
// # annotation strings, `"int"`/`"str"`/`"object"` literal matching, `|`     #
// # splitting, `starts_with("tuple[")`). It was deleted, not replaced.       #
// #                                                                          #
// # The call sites below are LEFT BROKEN ON PURPOSE. They are the map of     #
// # what must be rebuilt on the resolved AST — resolved bindings, canonical  #
// # `TypeNode`, and `assignable`/`equivalent` — or made to abstain.          #
// #                                                                          #
// # Restoring the deleted helper, vendoring a copy of it, or re-deriving a   #
// # type from source text anywhere below is FORBIDDEN.                       #
// #                                                                          #
// # Evidence and the failing tests that pin the real behaviour:              #
// #   docs/RULE-VALIDITY-REPORT.md                                           #
// #   crates/basilisk-checker/tests/legacy_annotation_text_parser_pin_tests.rs
// #   crates/basilisk-checker/tests/pep_spelling_invariance_pin_tests.rs     #
// ############################################################################

//! Implements [`assignment_compatibility`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! Recursive type-alias value matching for `assignment_compatibility`.
//!
//! Legacy type aliases such as
//!
//! ```python
//! Json = Union[None, int, str, float, list["Json"], dict[str, "Json"]]
//! j1: Json = [1, {"a": 1}]   # OK
//! j4: Json = {"a": 1, "b": 3j}  # E: complex is not a Json value
//! ```
//!
//! cannot be validated by the plain `is_assignable_to` check because the
//! annotation is a `Named` reference and the right-hand side is a literal
//! structure. This module resolves a bare alias name to its (possibly
//! recursive) definition and verifies whether the inferred RHS literal type
//! *positively* matches it.
//!
//! *Generic* alias references (`G[str]` where `G = list["G[T]" | T]`) are NOT
//! specialised any more: substituting the use-site arguments would require
//! splitting and rewriting the rendered reference text, which is banned
//! ([ASTREBUILD-LAW]). Those references now abstain — the caller stays
//! lenient and emits nothing ([ASTREBUILD-PHASE-RESOLVER]).
//!
//! **Positive-match semantics.** A value matches only when every part is
//! demonstrably compatible. `Unknown`/`Any` values do **not** positively match a
//! concrete target (the checker cannot prove compatibility), so genuinely
//! incompatible assignments keep firing — this preserves the true positives that
//! the recursive-alias fixtures expect.

use std::collections::{HashMap, HashSet};

use crate::span_util::slice_span;
use crate::types::InferredType;
use basilisk_resolver::{ResolvedModule, VariableInfo};

/// Maximum alias-expansion depth — a safety bound for self-referential aliases.
const MAX_DEPTH: u32 = 24;

/// A generic value alias such as `Name = list[... T ...]`, retaining the
/// `TypeVar` parameter names (lowercased) it binds so a use-site `Name[Arg]`
/// can be specialised before matching.
pub(super) struct GenericAlias {
    params: Vec<String>,
    def_text: String,
}

/// Alias-resolution context: legacy value aliases (`Union` or concrete
/// container bodies) plus generic (`TypeVar`-parameterised) aliases, both
/// keyed by lowercase base name.
pub(super) struct AliasCtx<'a> {
    pub(super) union: &'a HashMap<String, InferredType>,
    pub(super) generic: &'a HashMap<String, GenericAlias>,
}

/// Collect module-level value-style type aliases: `Union` definitions plus
/// concrete structural containers.
///
/// These are legacy aliases written without annotation, e.g. `Name = Union[...]`,
/// `Name = a | b | …`, or `Name = dict[tuple[str, str], str]`. Container-bodied
/// definitions that reference a module `TypeVar` are deliberately excluded —
/// [`collect_generic_aliases`] handles those with `TypeVar` substitution.
pub(super) fn collect_value_aliases(module: &ResolvedModule) -> HashMap<String, InferredType> {
    let typevars: HashSet<String> = module
        .typevar_calls
        .iter()
        .map(|tv| tv.name.to_ascii_lowercase())
        .collect();
    let mut aliases = HashMap::new();
    for var in &module.module_vars {
        if var.has_annotation {
            continue;
        }
        let Some(text) = alias_rhs_text(var, module) else {
            continue;
        };
        // NOT `resolver.resolve_text`: this table feeds a depth-limited
        // recursive matcher that needs the alias body's OWN shape, with
        // self-references intact as leaves. The cascade expands aliases
        // transparently and cuts the cycle to gradual `Unknown`, which
        // erases exactly the leaf the matcher recurses on — measured to cost
        // two recursive-alias acceptances (`fp_elimination_tests`).
        // Retiring this site means deleting the matcher in favour of the
        // cascade's own recursive-alias handling, not swapping the parser
        // ([NARROWPLAN-INTEGRATION] Step 7,
        // [#379](https://github.com/Nimblesite/Basilisk/issues/379)).
        let def = InferredType::from_annotation(text.trim());
        let include = match def {
            InferredType::Union(_) => true,
            InferredType::Dict(..)
            | InferredType::List(_)
            | InferredType::Set(_)
            | InferredType::Tuple(_) => {
                free_typevars(&text.to_ascii_lowercase(), &typevars).is_empty()
            }
            _ => false,
        };
        if include {
            let _ = aliases.insert(var.name.to_ascii_lowercase(), def);
        }
    }
    aliases
}

/// The module `TypeVar`s that appear as whole identifiers in `lowered`, in order
/// of first appearance, de-duplicated.
///
/// Equivalent to filtering `typevars` by `contains_word`, but scans the RHS
/// tokens once (`O(text)`) instead of testing every module `TypeVar` against the
/// text (`O(text * typevars)` — quadratic and allocation-heavy on
/// `TypeVar`-dense modules; see the `generics_basic` stress fixture). Appearance
/// order is deterministic, unlike the previous `HashSet` iteration order.
fn free_typevars(lowered: &str, typevars: &HashSet<String>) -> Vec<String> {
    let mut params = Vec::new();
    let mut seen = HashSet::new();
    for token in lowered.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
        if !token.is_empty() && typevars.contains(token) && seen.insert(token) {
            params.push(token.to_owned());
        }
    }
    params
}

/// Collect generic value aliases keyed by lowercase name, with the `TypeVar`
/// params each binds.
///
/// Two passes: roots whose body references a `TypeVar` (e.g.
/// `G = list["G[T]" | T]`), then specialisations that reference a root via a
/// subscript (e.g. `S = G[str]`) and therefore bind no params of their own.
///
/// The collected roots are no longer specialised — [`resolve_generic`]
/// abstains for any alias that binds params ([ASTREBUILD-LAW]) — but the
/// table still identifies which annotations name a value alias, so the
/// caller can stay lenient on them instead of misjudging the reference
/// through the ordinary assignability path.
pub(super) fn collect_generic_aliases(module: &ResolvedModule) -> HashMap<String, GenericAlias> {
    let typevars: HashSet<String> = module
        .typevar_calls
        .iter()
        .map(|tv| tv.name.to_ascii_lowercase())
        .collect();
    let mut generics: HashMap<String, GenericAlias> = HashMap::new();

    for var in &module.module_vars {
        if var.has_annotation {
            continue;
        }
        let Some(text) = alias_rhs_text(var, module) else {
            continue;
        };
        let lowered = text.to_ascii_lowercase();
        let params = free_typevars(&lowered, &typevars);
        if !params.is_empty() {
            let _ = generics.insert(
                var.name.to_ascii_lowercase(),
                GenericAlias {
                    params,
                    def_text: lowered,
                },
            );
        }
    }

    let mut specialised: Vec<(String, GenericAlias)> = Vec::new();
    for var in &module.module_vars {
        if var.has_annotation {
            continue;
        }
        let key = var.name.to_ascii_lowercase();
        if generics.contains_key(&key) {
            continue;
        }
        let Some(text) = alias_rhs_text(var, module) else {
            continue;
        };
        let lowered = text.to_ascii_lowercase();
        if generics.contains_key(alias_base(&lowered)) {
            specialised.push((
                key,
                GenericAlias {
                    params: Vec::new(),
                    def_text: lowered,
                },
            ));
        }
    }
    for (key, alias) in specialised {
        let _ = generics.insert(key, alias);
    }
    generics
}

/// If `declared_name` references a known value alias (union or generic), return
/// `Some(true)` when `value` positively matches it and `Some(false)` otherwise.
/// Returns `None` when the name is not a value alias, so the caller can fall
/// back to its ordinary assignability check.
#[expect(
    dead_code,
    reason = "orphaned by the deletion of the spelling-keyed `check_vars` pipeline; retained as the map for the identity-keyed rebuild ([ASTREBUILD-PHASE-TYPEEXPR])"
)]
pub(super) fn alias_value_assignable(
    value: &InferredType,
    declared_name: &str,
    ctx: &AliasCtx<'_>,
) -> Option<bool> {
    let base = alias_base(declared_name);
    if ctx.union.contains_key(base) || ctx.generic.contains_key(base) {
        return Some(match_named_target(value, declared_name, ctx, 0));
    }
    None
}

/// The trimmed RHS source text of an alias assignment, if non-empty.
///
/// A `Name = TypeAliasType("Name", body, type_params=(T,))` definition is NOT a
/// textual alias body: its body is the call's SECOND ARGUMENT, and the call
/// expression itself denotes no type at all. Matching a value against that text
/// asks whether e.g. `1` matches `typealiastype("goodalias4", …)`, which can
/// only ever answer "no" — a false positive on every valid use of a
/// `TypeAliasType` alias. These aliases are resolved by the
/// [TYPEINF-ANNOTATION-RESOLUTION] cascade instead, so they are excluded here
/// rather than approximated.
fn alias_rhs_text(var: &VariableInfo, module: &ResolvedModule) -> Option<String> {
    if is_type_alias_type_call(var, module) {
        return None;
    }
    let rhs_span = var.rhs_span?;
    let rhs_text = slice_span(&module.source, rhs_span)?.trim();
    (!rhs_text.is_empty()).then(|| rhs_text.to_owned())
}

/// Whether `var` is the LHS of a `TypeAliasType(...)` call the resolver
/// recognised (structural, never a text match on the RHS).
fn is_type_alias_type_call(var: &VariableInfo, module: &ResolvedModule) -> bool {
    module
        .type_alias_type_calls
        .iter()
        .any(|call| call.lhs_name == var.name)
}

/// Returns `true` when `value` positively matches the (possibly recursive)
/// alias `target`. See the module docs for the positive-match contract.
#[expect(
    dead_code,
    reason = "orphaned by the deletion of the spelling-keyed `check_vars` pipeline; retained as the map for the identity-keyed rebuild ([ASTREBUILD-PHASE-TYPEEXPR])"
)]
pub(super) fn alias_assignable(
    value: &InferredType,
    target: &InferredType,
    ctx: &AliasCtx<'_>,
    depth: u32,
) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }

    // `Never` is the empty type (e.g. an empty list/dict literal element) and
    // vacuously matches any target.
    if matches!(value, InferredType::Never) {
        return true;
    }

    // A union value matches only when every member matches the target.
    if let InferredType::Union(members) = value {
        return members
            .iter()
            .all(|member| alias_assignable(member, target, ctx, depth + 1));
    }

    match target {
        InferredType::Named(name) => match_named_target(value, name, ctx, depth),
        InferredType::Union(branches) => branches
            .iter()
            .any(|branch| alias_assignable(value, branch, ctx, depth + 1)),
        InferredType::List(elem) => match value {
            InferredType::List(v) => alias_assignable(v, elem, ctx, depth + 1),
            _ => false,
        },
        InferredType::Set(elem) => match value {
            InferredType::Set(v) => alias_assignable(v, elem, ctx, depth + 1),
            _ => false,
        },
        InferredType::Dict(key, val) => match value {
            InferredType::Dict(value_key, value_val) => {
                alias_assignable(value_key, key, ctx, depth + 1)
                    && alias_assignable(value_val, val, ctx, depth + 1)
            }
            _ => false,
        },
        InferredType::Tuple(target_elems) => match_tuple_target(value, target_elems, ctx, depth),
        InferredType::Any => true,
        _ => positive_base_match(value, target),
    }
}

/// Match a value against a `Named` target, resolving union aliases and generic
/// alias specialisations.
#[expect(
    dead_code,
    reason = "orphaned by the deletion of the spelling-keyed `check_vars` pipeline; retained as the map for the identity-keyed rebuild ([ASTREBUILD-PHASE-TYPEEXPR])"
)]
fn match_named_target(value: &InferredType, name: &str, ctx: &AliasCtx<'_>, depth: u32) -> bool {
    let base = alias_base(name);
    if let Some(def) = ctx.union.get(base) {
        return alias_assignable(value, def, ctx, depth + 1);
    }
    if let Some(generic) = ctx.generic.get(base) {
        return match resolve_generic(generic) {
            Some(resolved) => alias_assignable(value, &resolved, ctx, depth + 1),
            // A parameterised alias cannot be specialised here (see
            // [`resolve_generic`]): the checker cannot prove a mismatch, so
            // stay lenient and emit nothing.
            None => true,
        };
    }

    // A non-alias class name: a value positively matches only when it is the
    // same named type (by base name).
    match value {
        InferredType::Named(value_name) => alias_base(value_name) == base,
        _ => false,
    }
}

/// Expand a generic alias into a matchable type.
///
/// A parameterised alias (`G[str]`) would need its use-site subscript
/// arguments resolved and substituted, but this matcher only holds the
/// rendered reference text, which may not lawfully be split or rewritten
/// ([ASTREBUILD-LAW]). Such references return `None` and the caller stays
/// lenient ([ASTREBUILD-PHASE-RESOLVER]); the matcher itself dies with the
/// alias tables in [NARROWPLAN-INTEGRATION] Step 7. A parameterless
/// specialisation (`S = G[str]` collected as its own entry) expands its
/// stored definition directly.
#[expect(
    dead_code,
    reason = "orphaned by the deletion of the spelling-keyed `check_vars` pipeline; retained as the map for the identity-keyed rebuild ([ASTREBUILD-PHASE-TYPEEXPR])"
)]
fn resolve_generic(alias: &GenericAlias) -> Option<InferredType> {
    if !alias.params.is_empty() {
        return None;
    }
    Some(InferredType::from_annotation(&alias.def_text))
}

/// Match a value against a tuple target, handling the homogeneous
/// `tuple[X, ...]` form (the parser stores the `...` terminator as `Named`).
#[expect(
    dead_code,
    reason = "orphaned by the deletion of the spelling-keyed `check_vars` pipeline; retained as the map for the identity-keyed rebuild ([ASTREBUILD-PHASE-TYPEEXPR])"
)]
fn match_tuple_target(
    value: &InferredType,
    target_elems: &[InferredType],
    ctx: &AliasCtx<'_>,
    depth: u32,
) -> bool {
    let InferredType::Tuple(value_elems) = value else {
        return false;
    };
    if let [elem, InferredType::Named(terminator)] = target_elems {
        if terminator == "..." {
            return value_elems
                .iter()
                .all(|ve| alias_assignable(ve, elem, ctx, depth + 1));
        }
    }
    value_elems.len() == target_elems.len()
        && value_elems
            .iter()
            .zip(target_elems.iter())
            .all(|(ve, te)| alias_assignable(ve, te, ctx, depth + 1))
}

/// Positive structural match for primitive base types. Notably `Unknown`/`Any`
/// values do NOT match a concrete base type — the checker cannot prove it.
#[expect(
    dead_code,
    reason = "orphaned by the deletion of the spelling-keyed `check_vars` pipeline; retained as the map for the identity-keyed rebuild ([ASTREBUILD-PHASE-TYPEEXPR])"
)]
fn positive_base_match(value: &InferredType, target: &InferredType) -> bool {
    use crate::types::LiteralValue;
    use InferredType::{Bool, Bytes, Float, Int, Literal, LiteralString, None_, Str};
    match target {
        Int => matches!(
            value,
            Int | Bool | Literal(LiteralValue::Int(_) | LiteralValue::Bool(_))
        ),
        Float => matches!(
            value,
            Float | Int | Bool | Literal(LiteralValue::Float(_) | LiteralValue::Int(_))
        ),
        Str => matches!(value, Str | LiteralString | Literal(LiteralValue::Str(_))),
        Bool => matches!(value, Bool | Literal(LiteralValue::Bool(_))),
        Bytes => matches!(value, Bytes | Literal(LiteralValue::Bytes(_))),
        None_ => matches!(value, None_),
        _ => value == target,
    }
}

// ##########################################################################
// # DELETED BODY — `alias_base`. DO NOT RESTORE IT. DO NOT SUBSTITUTE A    #
// # PLACEHOLDER THAT RETURNS THE INPUT UNCHANGED.                          #
// #                                                                        #
// # It read:                                                               #
// #   let trimmed = name.trim_matches(|c| c == '"' || c == '\'');          #
// #   trimmed.split('[').next().unwrap_or(trimmed).trim()                  #
// #                                                                        #
// # Three spelling operations in two lines: unquoting a FORWARD REFERENCE  #
// # by stripping quote characters, taking a generic head by splitting at a #
// # bracket, and trimming whitespace that a formatter controls. A quoted   #
// # annotation is an `Expr::StringLiteral` whose contents parse as a type  #
// # expression — the binding table already answers it via                  #
// # `form_of_quoted_annotation`. None of it is a text problem.             #
// #                                                                        #
// # Pinned by: tests/no_type_spelling_surgery_tests.rs                     #
// ##########################################################################

/// DELETED — panics. The signature survives only so its callers stay visible
/// as the rebuild map; see the banner above.
fn alias_base(_name: &str) -> &str {
    panic!(
        "basilisk-checker: `alias_base` was DELETED because it found an alias's \
         lookup key by stripping quote characters and splitting a RENDERED type at \
         `[`. It panics because the real implementation — resolving the annotation \
         expression (including quoted forward references) through the binding table \
         — DOES NOT EXIST YET. Do not restore the string surgery and do not return \
         the input unchanged in its place."
    )
}
