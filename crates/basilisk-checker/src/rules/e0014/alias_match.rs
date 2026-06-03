//! Implements [BSK-E0014] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! Recursive type-alias value matching for BSK-E0014.
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
//! **Positive-match semantics.** A value matches only when every part is
//! demonstrably compatible. `Unknown`/`Any` values do **not** positively match a
//! concrete target (the checker cannot prove compatibility), so genuinely
//! incompatible assignments keep firing — this preserves the true positives that
//! the recursive-alias fixtures expect.

use std::collections::HashMap;

use crate::span_util::slice_span;
use crate::types::InferredType;
use basilisk_resolver::{ResolvedModule, VariableInfo};

/// Maximum alias-expansion depth — a safety bound for self-referential aliases.
const MAX_DEPTH: u32 = 24;

/// Collect module-level value-style type aliases whose definition is a `Union`.
///
/// These are legacy aliases written as `Name = Union[...]` or `Name = a | b | …`
/// (no annotation). Restricting to `Union` definitions deliberately excludes
/// generic (`list[...]`-bodied) aliases that would need TypeVar substitution.
pub(super) fn collect_union_aliases(module: &ResolvedModule) -> HashMap<String, InferredType> {
    let mut aliases = HashMap::new();
    for var in &module.module_vars {
        if var.has_annotation {
            continue;
        }
        let Some(def) = alias_definition(var, &module.source) else {
            continue;
        };
        if matches!(def, InferredType::Union(_)) {
            let _ = aliases.insert(var.name.to_ascii_lowercase(), def);
        }
    }
    aliases
}

/// Parse a variable's RHS source text into the alias definition type.
fn alias_definition(var: &VariableInfo, source: &str) -> Option<InferredType> {
    let rhs_span = var.rhs_span?;
    let rhs_text = slice_span(source, rhs_span)?.trim();
    if rhs_text.is_empty() {
        return None;
    }
    Some(InferredType::from_annotation(rhs_text))
}

/// Returns `true` when `value` positively matches the (possibly recursive)
/// alias `target`. See the module docs for the positive-match contract.
pub(super) fn alias_assignable(
    value: &InferredType,
    target: &InferredType,
    aliases: &HashMap<String, InferredType>,
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
            .all(|member| alias_assignable(member, target, aliases, depth + 1));
    }

    match target {
        InferredType::Named(name) => match_named_target(value, name, aliases, depth),
        InferredType::Union(branches) => branches
            .iter()
            .any(|branch| alias_assignable(value, branch, aliases, depth + 1)),
        InferredType::List(elem) => match value {
            InferredType::List(v) => alias_assignable(v, elem, aliases, depth + 1),
            _ => false,
        },
        InferredType::Set(elem) => match value {
            InferredType::Set(v) => alias_assignable(v, elem, aliases, depth + 1),
            _ => false,
        },
        InferredType::Dict(key, val) => match value {
            InferredType::Dict(vkey, vval) => {
                alias_assignable(vkey, key, aliases, depth + 1)
                    && alias_assignable(vval, val, aliases, depth + 1)
            }
            _ => false,
        },
        InferredType::Tuple(target_elems) => match_tuple_target(value, target_elems, aliases, depth),
        InferredType::Any => true,
        _ => positive_base_match(value, target),
    }
}

/// Match a value against a `Named` target, resolving alias references and the
/// `Mapping[K, V]` ABC (which the parser leaves as a `Named`).
fn match_named_target(
    value: &InferredType,
    name: &str,
    aliases: &HashMap<String, InferredType>,
    depth: u32,
) -> bool {
    // `Mapping[K, V]` arrives as a Named; treat it structurally like a dict.
    if let Some(dict) = parse_mapping_named(name) {
        return alias_assignable(value, &dict, aliases, depth + 1);
    }

    let base = alias_base(name);
    if let Some(def) = aliases.get(base) {
        return alias_assignable(value, def, aliases, depth + 1);
    }

    // A non-alias class name: a value positively matches only when it is the
    // same named type (by base name).
    match value {
        InferredType::Named(value_name) => alias_base(value_name) == base,
        _ => false,
    }
}

/// Match a value against a tuple target, handling the homogeneous
/// `tuple[X, ...]` form (the parser stores the `...` terminator as `Named`).
fn match_tuple_target(
    value: &InferredType,
    target_elems: &[InferredType],
    aliases: &HashMap<String, InferredType>,
    depth: u32,
) -> bool {
    let InferredType::Tuple(value_elems) = value else {
        return false;
    };
    if let [elem, InferredType::Named(terminator)] = target_elems {
        if terminator == "..." {
            return value_elems
                .iter()
                .all(|ve| alias_assignable(ve, elem, aliases, depth + 1));
        }
    }
    value_elems.len() == target_elems.len()
        && value_elems
            .iter()
            .zip(target_elems.iter())
            .all(|(ve, te)| alias_assignable(ve, te, aliases, depth + 1))
}

/// Positive structural match for primitive base types. Notably `Unknown`/`Any`
/// values do NOT match a concrete base type — the checker cannot prove it.
fn positive_base_match(value: &InferredType, target: &InferredType) -> bool {
    use InferredType::{Bool, Bytes, Float, Int, Literal, LiteralString, None_, Str};
    use crate::types::LiteralValue;
    match target {
        Int => matches!(value, Int | Bool | Literal(LiteralValue::Int(_) | LiteralValue::Bool(_))),
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
/// and any `[...]` subscript, returning the bare lowercase name.
fn alias_base(name: &str) -> &str {
    let trimmed = name.trim_matches(|c| c == '"' || c == '\'');
    trimmed.split('[').next().unwrap_or(trimmed).trim()
}

/// Parse a `mapping[K, V]` Named into a `Dict(K, V)` so it can be matched
/// structurally against a dict literal value.
fn parse_mapping_named(name: &str) -> Option<InferredType> {
    let inner = name.strip_prefix("mapping[")?.strip_suffix(']')?;
    let parts = crate::types_parsing::split_type_params(inner);
    if parts.len() != 2 {
        return None;
    }
    let key = InferredType::from_annotation(parts.first().map_or("", |s| s.trim()));
    let val = InferredType::from_annotation(parts.get(1).map_or("", |s| s.trim()));
    Some(InferredType::Dict(Box::new(key), Box::new(val)))
}
