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
//! and *generic* recursive aliases such as
//!
//! ```python
//! G = list["G[T]" | T]            # T = TypeVar("T", str, int)
//! S = G[str]
//! g1: S = ["hi", ["hi", "hi"]]    # OK
//! g3: G[str] = ["hi", [2.4]]      # E: float is not a `str` leaf
//! ```
//!
//! cannot be validated by the plain `is_assignable_to` check because the
//! annotation is a `Named` reference and the right-hand side is a literal
//! structure. This module resolves a bare or specialised alias name to its
//! (possibly recursive) definition — substituting `TypeVar` arguments for
//! generic aliases — and verifies whether the inferred RHS literal type
//! *positively* matches it.
//!
//! **Positive-match semantics.** A value matches only when every part is
//! demonstrably compatible. `Unknown`/`Any` values do **not** positively match a
//! concrete target (the checker cannot prove compatibility), so genuinely
//! incompatible assignments keep firing — this preserves the true positives that
//! the recursive-alias fixtures expect.

use std::collections::{HashMap, HashSet};

use super::callable_check::replace_word;
use crate::rules::shared::split_top_level_commas;
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
        if var.has_annotation && !is_typealias_annotation(var, &module.source) {
            continue;
        }
        let Some(text) = alias_rhs_text(var, &module.source) else {
            continue;
        };
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
/// Unlike [`collect_value_aliases`], annotated assignments are skipped even
/// when the annotation is an explicit `TypeAlias`. Substituting into a generic
/// alias is textual here, which is sound for the container bodies this pass
/// targets but not for a `Callable` body parameterised by a `ParamSpec` —
/// `Callback: TypeAlias = Callable[P, str]` used as `Callback[...]` needs real
/// `ParamSpec` semantics, and approximating it produced false positives on the
/// conformance suite's `callables_annotation` / `callables_subtyping` fixtures.
/// Those forms are left to the callable-compatibility path that does model them.
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
        let Some(text) = alias_rhs_text(var, &module.source) else {
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
        let Some(text) = alias_rhs_text(var, &module.source) else {
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

/// Returns `true` when `var` carries an explicit `TypeAlias` annotation, i.e.
/// `Name: TypeAlias = ...` (also `typing.TypeAlias`). Such assignments are
/// value aliases too and must be collected despite `has_annotation` being set.
///
/// The annotation may be written as a string — `Name: "TypeAlias" = ...` is the
/// same declaration to a type checker, since any annotation may appear as a
/// forward reference ([annotation expressions](https://typing.python.org/en/latest/spec/annotations.html#string-annotations)).
/// [`alias_base`] strips those quotes for the same reason, so the two agree on
/// what a name is.
fn is_typealias_annotation(var: &VariableInfo, source: &str) -> bool {
    let Some(span) = var.annotation_span else {
        return false;
    };
    let Some(text) = slice_span(source, span) else {
        return false;
    };
    let unquoted = alias_base(text.trim());
    let base = unquoted.rsplit('.').next().unwrap_or(unquoted);
    base == "TypeAlias"
}

/// The trimmed RHS source text of an alias assignment, if non-empty.
fn alias_rhs_text(var: &VariableInfo, source: &str) -> Option<String> {
    let rhs_span = var.rhs_span?;
    let rhs_text = slice_span(source, rhs_span)?.trim();
    (!rhs_text.is_empty()).then(|| rhs_text.to_owned())
}

/// Returns `true` when `value` positively matches the (possibly recursive)
/// alias `target`. See the module docs for the positive-match contract.
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

/// Match a value against a `Named` target, resolving union aliases, generic
/// alias specialisations, and the `Mapping[K, V]` ABC (left as a `Named`).
fn match_named_target(value: &InferredType, name: &str, ctx: &AliasCtx<'_>, depth: u32) -> bool {
    // `Mapping[K, V]` arrives as a Named; treat it structurally like a dict.
    if let Some(dict) = parse_mapping_named(name) {
        return alias_assignable(value, &dict, ctx, depth + 1);
    }

    let base = alias_base(name);
    if let Some(def) = ctx.union.get(base) {
        return alias_assignable(value, def, ctx, depth + 1);
    }
    if let Some(generic) = ctx.generic.get(base) {
        return match resolve_generic(name, generic) {
            Some(resolved) => alias_assignable(value, &resolved, ctx, depth + 1),
            // Arity mismatch (e.g. a bare generic alias used without args):
            // the checker cannot prove a mismatch, so stay lenient.
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

/// Specialise a generic alias use-site (`Name[Arg, …]`) into a concrete type by
/// substituting its `TypeVar` params. Returns `None` on arity mismatch.
fn resolve_generic(name: &str, alias: &GenericAlias) -> Option<InferredType> {
    let args = subscript_args(name);
    let mut text = alias.def_text.clone();
    if alias.params.len() == args.len() {
        for (param, arg) in alias.params.iter().zip(args.iter()) {
            text = replace_word(&text, param, arg);
        }
    } else if !alias.params.is_empty() {
        return None;
    }
    Some(InferredType::from_annotation(&text))
}

/// Extract the top-level subscript arguments from a `Name[A, B]` reference.
fn subscript_args(name: &str) -> Vec<String> {
    let trimmed = name.trim().trim_matches(|c| c == '"' || c == '\'');
    let Some(open) = trimmed.find('[') else {
        return Vec::new();
    };
    let Some(close) = trimmed.rfind(']') else {
        return Vec::new();
    };
    if close <= open + 1 {
        return Vec::new();
    }
    split_top_level_commas(&trimmed[open + 1..close])
        .into_iter()
        .map(|arg| arg.trim().to_owned())
        .collect()
}

/// Match a value against a tuple target, handling the homogeneous
/// `tuple[X, ...]` form (the parser stores the `...` terminator as `Named`).
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

/// Extract the alias-lookup base from a `Named` text: strip surrounding quotes
/// and any `[...]` subscript, returning the bare name.
fn alias_base(name: &str) -> &str {
    let trimmed = name.trim_matches(|c| c == '"' || c == '\'');
    trimmed.split('[').next().unwrap_or(trimmed).trim()
}

/// Parse a `mapping[K, V]` Named into a `Dict(K, V)` so it can be matched
/// structurally against a dict literal value.
fn parse_mapping_named(name: &str) -> Option<InferredType> {
    let inner = name.strip_prefix("mapping[")?.strip_suffix(']')?;
    let (key, val) = crate::types_parsing::parse_key_value_args(inner)?;
    Some(InferredType::Dict(Box::new(key), Box::new(val)))
}
