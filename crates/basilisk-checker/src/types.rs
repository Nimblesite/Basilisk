//! Implements [TYPEINF-OVERVIEW]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#typeinf-overview
//! Type representation for Basilisk's type inference engine.
//!
//! Annotation parsing logic lives in [`super::types_parsing`].

use std::fmt;

/// Represents an inferred type from Basilisk's type inference engine.
#[derive(Debug, Clone, PartialEq)]
pub enum InferredType {
    /// Integer type (`int`)
    Int,
    /// String type (`str`)
    Str,
    /// Float type (`float`)
    Float,
    /// Boolean type (`bool`)
    Bool,
    /// Bytes type (`bytes`)
    Bytes,
    /// None type (`None`)
    None_,
    /// Literal value type (`Literal[value]`)
    Literal(LiteralValue),
    /// List type (`list[T]`)
    List(Box<InferredType>),
    /// Dictionary type (`dict[K, V]`)
    Dict(Box<InferredType>, Box<InferredType>),
    /// Set type (`set[T]`)
    Set(Box<InferredType>),
    /// Tuple type (`tuple[T1, T2, ...]`)
    Tuple(Vec<InferredType>),
    /// Union type (`T1 | T2`)
    Union(Vec<InferredType>),
    /// Optional type (`Optional[T]` or `T | None`)
    Optional(Box<InferredType>),
    /// Callable type (`Callable[[params...], return]` or `Callable[..., return]`)
    Callable(CallableInfo),
    /// Any type (`Any`) - explicit escape hatch
    Any,
    /// Never type (`Never`) - bottom type, no values
    Never,
    /// Unknown type - used when type cannot be determined
    Unknown,
    /// `LiteralString` — any string literal or value known to be a literal string
    LiteralString,
    /// Named type (`ClassName`) - fallback for named types not yet resolved
    Named(String),
    /// `TypeForm[T]` — represents a type form object for type `T` (PEP 747).
    /// The inner type is what the type form represents (e.g. `TypeForm[int]`
    /// means a type form that represents `int`).
    TypeForm(Box<InferredType>),
}

/// Represents a callable type's parameter and return type information.
#[derive(Debug, Clone, PartialEq)]
pub struct CallableInfo {
    /// Parameter types (empty for `Callable[..., R]`).
    pub param_types: Vec<InferredType>,
    /// Return type.
    pub return_type: Box<InferredType>,
}

/// Represents a literal value for literal type inference.
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    /// Integer literal value
    Int(i64),
    /// String literal value
    Str(String),
    /// Float literal value
    Float(f64),
    /// Boolean literal value
    Bool(bool),
    /// Bytes literal value
    Bytes(Vec<u8>),
}

impl fmt::Display for InferredType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InferredType::Int => write!(f, "int"),
            InferredType::Str => write!(f, "str"),
            InferredType::Float => write!(f, "float"),
            InferredType::Bool => write!(f, "bool"),
            InferredType::Bytes => write!(f, "bytes"),
            InferredType::None_ => write!(f, "None"),
            InferredType::Literal(lit) => write!(f, "Literal[{lit}]"),
            InferredType::List(elem_type) => write!(f, "list[{elem_type}]"),
            InferredType::Dict(key_type, value_type) => {
                write!(f, "dict[{key_type}, {value_type}]")
            }
            InferredType::Set(elem_type) => write!(f, "set[{elem_type}]"),
            InferredType::Tuple(elem_types) => {
                write!(f, "tuple[")?;
                for (i, elem_type) in elem_types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{elem_type}")?;
                }
                write!(f, "]")
            }
            InferredType::Union(types) => {
                for (i, t) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{t}")?;
                }
                Ok(())
            }
            InferredType::Optional(inner) => write!(f, "Optional[{inner}]"),
            InferredType::Callable(info) => {
                write!(f, "Callable[[")?;
                for (i, param) in info.param_types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{param}")?;
                }
                write!(f, "], {}]", info.return_type)
            }
            InferredType::Any => write!(f, "Any"),
            InferredType::Never => write!(f, "Never"),
            InferredType::Unknown => write!(f, "Unknown"),
            InferredType::LiteralString => write!(f, "LiteralString"),
            InferredType::Named(name) => write!(f, "{name}"),
            InferredType::TypeForm(inner) => match inner.as_ref() {
                InferredType::Any => write!(f, "TypeForm"),
                other => write!(f, "TypeForm[{other}]"),
            },
        }
    }
}

impl fmt::Display for LiteralValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LiteralValue::Int(val) => write!(f, "{val}"),
            LiteralValue::Str(val) => write!(f, "\"{val}\""),
            LiteralValue::Float(val) => write!(f, "{val}"),
            LiteralValue::Bool(val) => write!(f, "{val}"),
            LiteralValue::Bytes(val) => {
                let lossy = String::from_utf8_lossy(val);
                write!(f, "b\"{lossy}\"")
            }
        }
    }
}

impl InferredType {
    /// Creates a union of two types, flattening nested unions.
    #[must_use]
    pub fn union(a: InferredType, b: InferredType) -> InferredType {
        // Helper to flatten a type into the vector
        fn flatten_into(ty: InferredType, vec: &mut Vec<InferredType>) {
            match ty {
                InferredType::Union(mut inner_types) => {
                    for inner_ty in inner_types.drain(..) {
                        flatten_into(inner_ty, vec);
                    }
                }
                other => vec.push(other),
            }
        }

        // Handle Never specially: Never ∪ T = T
        if matches!(a, InferredType::Never) {
            return b;
        }
        if matches!(b, InferredType::Never) {
            return a;
        }

        // Flatten both sides into vectors of types
        let mut types = Vec::new();
        flatten_into(a, &mut types);
        flatten_into(b, &mut types);

        // Deduplicate types
        let mut deduplicated = Vec::new();
        for ty in types {
            if !deduplicated.contains(&ty) {
                deduplicated.push(ty);
            }
        }

        // If only one type remains, return it directly (not wrapped in Union)
        match deduplicated.len() {
            0 => InferredType::Never, // Should not happen due to Never handling above
            1 => match deduplicated.into_iter().next() {
                Some(ty) => ty,
                None => InferredType::Never, // len()==1 guarantees Some
            },
            _ => InferredType::Union(deduplicated),
        }
    }

    /// Returns true if this type is assignable to the other type.
    #[must_use]
    pub fn is_assignable_to(&self, other: &InferredType) -> bool {
        match (self, other) {
            // Any is assignable to/from everything (PEP 484).
            // Unknown means we cannot determine the type — assume compatible to avoid false positives.
            // Never is the bottom type — assignable to everything.
            (InferredType::Any | InferredType::Never | InferredType::Unknown, _)
            | (_, InferredType::Any | InferredType::Unknown) => true,
            // Same types are assignable
            (a, b) if a == b => true,
            // Int→float widening, Literal assignable to base type,
            // string types assignable to LiteralString, LiteralString to str
            (
                InferredType::Int | InferredType::Literal(LiteralValue::Float(_)),
                InferredType::Float,
            )
            | (
                InferredType::Literal(LiteralValue::Int(_)),
                InferredType::Int | InferredType::Float,
            )
            | (
                InferredType::Literal(LiteralValue::Str(_)) | InferredType::LiteralString,
                InferredType::Str,
            )
            | (
                InferredType::Literal(LiteralValue::Bool(_)),
                InferredType::Bool | InferredType::Int,
            )
            | (InferredType::Literal(LiteralValue::Bytes(_)), InferredType::Bytes)
            | (
                InferredType::Str | InferredType::Literal(LiteralValue::Str(_)),
                InferredType::LiteralString,
            )
            // None is always assignable to Optional[T]
            | (InferredType::None_, InferredType::Optional(_)) => true,
            // `None` satisfies `Hashable` (it defines `__hash__`). The annotation
            // parser lowercases names, so the ABC arrives as `Named("hashable")`.
            (InferredType::None_, InferredType::Named(name)) if name == "hashable" => true,
            // Optional types are assignable to their non-optional counterparts
            (InferredType::Optional(inner), other) => inner.is_assignable_to(other),
            (inner, InferredType::Optional(other)) => inner.is_assignable_to(other),
            // Union types require all variants to be assignable
            (InferredType::Union(types), other) => types.iter().all(|t| t.is_assignable_to(other)),
            (inner, InferredType::Union(types)) => types.iter().any(|t| inner.is_assignable_to(t)),
            // Container types require element type assignability.
            // List and Set cannot use or-patterns — that would incorrectly allow cross-matching.
            (InferredType::List(a), InferredType::List(b))
            | (InferredType::Set(a), InferredType::Set(b)) => a.is_assignable_to(b),
            (InferredType::Dict(a_key, a_val), InferredType::Dict(b_key, b_val)) => {
                a_key.is_assignable_to(b_key) && a_val.is_assignable_to(b_val)
            }
            (InferredType::Tuple(a), InferredType::Tuple(b)) => {
                // A target with an unpacked `*tuple[...]` / `*Ts` segment (PEP 646)
                // needs prefix/middle/suffix matching, not positional equality.
                if b.iter().any(is_unpacked_tuple_elem) {
                    return tuple_assignable_with_star(a, b);
                }
                match (homogeneous_tuple_elem(a), homogeneous_tuple_elem(b)) {
                    // Target `tuple[X, ...]` (PEP 484 homogeneous variable-length):
                    // a source `tuple[Y, ...]` matches when `Y` is assignable to `X`.
                    (Some(a_elem), Some(b_elem)) => a_elem.is_assignable_to(b_elem),
                    // Target `tuple[X, ...]`: every element of a fixed-length source
                    // tuple must be assignable to `X` (empty tuple is vacuously valid).
                    (None, Some(b_elem)) => a.iter().all(|elem| elem.is_assignable_to(b_elem)),
                    // Target is fixed-length: a variable-length source cannot
                    // satisfy it — EXCEPT `tuple[Any, ...]` / `tuple[Unknown, ...]`,
                    // which are bidirectionally compatible with any tuple (PEP 484
                    // gradual typing).  Unresolved `Named` elements (e.g. types from
                    // unresolvable imports) are gradual too, not provable mismatches.
                    (Some(source_elem), None) => {
                        matches!(
                            source_elem,
                            InferredType::Any | InferredType::Unknown | InferredType::Named(_)
                        )
                    }
                    // Both fixed-length: require equal arity and positional assignability.
                    (None, None) => {
                        a.len() == b.len()
                            && a.iter()
                                .zip(b.iter())
                                .all(|(a_elem, b_elem)| a_elem.is_assignable_to(b_elem))
                    }
                }
            }
            // Callable type assignability
            (InferredType::Callable(a), InferredType::Callable(b)) => {
                // Check return type compatibility (covariant - source return must be assignable to target return)
                // Special case: if source return type is Unknown, we can't verify compatibility
                // This happens with lambda expressions where we can't infer the return type
                // We should be conservative and return false unless target return type is Any or Unknown
                match (&*a.return_type, &*b.return_type) {
                    (InferredType::Unknown, _)
                        if !matches!(
                            &*b.return_type,
                            InferredType::Any | InferredType::Unknown
                        ) =>
                    {
                        // Source has unknown return type, target has known return type
                        // This is unsafe - we don't know if they're compatible
                        return false;
                    }
                    _ => {
                        if !a.return_type.is_assignable_to(&b.return_type) {
                            return false;
                        }
                    }
                }

                // Handle ellipsis/arbitrary parameters (empty param_types means `...`)
                if a.param_types.is_empty() || b.param_types.is_empty() {
                    // If target accepts arbitrary parameters (`...`), any callable is assignable
                    // If source has arbitrary parameters, it can only be assigned to target with arbitrary parameters
                    // or if target has specific parameter types that match the source's capabilities
                    // For now, we allow if either has empty param_types (simplified)
                    return true;
                }

                // Check parameter count
                if a.param_types.len() != b.param_types.len() {
                    return false;
                }

                // Check parameter type compatibility (contravariant - target param must be assignable to source param)
                for (source_param, target_param) in a.param_types.iter().zip(b.param_types.iter()) {
                    if !target_param.is_assignable_to(source_param) {
                        return false;
                    }
                }

                true
            }
            // TypeForm covariance: TypeForm[S] is assignable to TypeForm[T] if S is assignable to T.
            (InferredType::TypeForm(inner_a), InferredType::TypeForm(inner_b)) => {
                inner_a.is_assignable_to(inner_b)
            }
            // Named types with the same base name (before `[`) are assumed compatible.
            // Without full generic variance analysis we cannot determine if
            // `Foo[int]` is assignable to `Foo[float]`, so we avoid false positives.
            (InferredType::Named(a_name), InferredType::Named(b_name)) => {
                let a_base = a_name.split('[').next().unwrap_or(a_name);
                let b_base = b_name.split('[').next().unwrap_or(b_name);
                a_base == b_base
            }
            // Ellipsis (`...`) parsed as Named is compatible when it appears
            // inside Callable parameter lists (e.g. `Callable[..., T]`).
            // For tuple annotations, `...` has special semantics that need
            // structural checking, so we don't treat it as universally compatible.
            _ => false,
        }
    }
}

/// Returns the element type `X` when `elems` is the homogeneous variable-length
/// tuple form `tuple[X, ...]`.
///
/// Implements [TYPEINF-OVERVIEW]. The annotation parser ([`super::types_parsing`])
/// represents the `...` terminator as `Named("...")`, so `tuple[str, ...]` becomes
/// `Tuple([Str, Named("...")])`. Distinguishing this from a fixed-length tuple is
/// what lets a literal `(a, b, c)` widen to `tuple[X, ...]` (PEP 484).
fn homogeneous_tuple_elem(elems: &[InferredType]) -> Option<&InferredType> {
    match elems {
        [elem, InferredType::Named(terminator)] if terminator == "..." => Some(elem),
        _ => None,
    }
}

/// Returns `true` when a tuple element is an unpacked variadic segment — either
/// `*tuple[...]` or a `*Ts` `TypeVarTuple`. The annotation parser stores these as
/// `Named` text beginning with `*`.
fn is_unpacked_tuple_elem(elem: &InferredType) -> bool {
    matches!(elem, InferredType::Named(name) if name.starts_with('*'))
}

/// What an unpacked `*tuple[...]` / `*Ts` segment consumes from a source tuple.
enum StarSegment {
    /// `*tuple[X, ...]` or `*Ts` — zero or more elements, each assignable to the
    /// element type (`None` ⇒ any, for `*Ts` / `*tuple[Any, ...]`).
    Variadic(Option<InferredType>),
    /// `*tuple[X, Y]` — a fixed run of elements consumed positionally.
    Fixed(Vec<InferredType>),
}

/// Parse an unpacked tuple element (`Named("*tuple[...]")` / `Named("*ts")`).
fn parse_star_segment(name: &str) -> StarSegment {
    let Some(inner) = name
        .strip_prefix("*tuple[")
        .and_then(|rest| rest.strip_suffix(']'))
    else {
        // `*Ts` (a TypeVarTuple) — any number of elements of any type.
        return StarSegment::Variadic(None);
    };
    let parts = crate::types_parsing::split_type_params(inner);
    if matches!(parts.last(), Some(last) if last.trim() == "...") {
        // `*tuple[X, ...]` — homogeneous, zero or more of `X`.
        let elem = parts
            .first()
            .map(|p| InferredType::from_annotation(p.trim()));
        return StarSegment::Variadic(elem);
    }
    StarSegment::Fixed(
        parts
            .iter()
            .map(|p| InferredType::from_annotation(p.trim()))
            .collect(),
    )
}

/// Match a fixed-length source tuple against a target tuple that contains a
/// single unpacked `*tuple[...]` / `*Ts` segment (PEP 646), using
/// prefix/middle/suffix decomposition.
fn tuple_assignable_with_star(source: &[InferredType], target: &[InferredType]) -> bool {
    // Unhandled shapes return `true` (assignable) rather than `false`: this is a
    // best-effort matcher and must never manufacture a false positive. Only a
    // pattern we actually decompose may return `false` (a real mismatch).
    //
    // A variadic source, multiple unpacked segments, or a `*Ts` we can't read
    // all need full PEP 646 unification — be permissive.
    if source.iter().any(is_unpacked_tuple_elem) {
        return true;
    }
    // A homogeneous variadic source (`tuple[X, ...]`) has unknown length —
    // prefix/middle/suffix decomposition cannot prove a mismatch.
    if homogeneous_tuple_elem(source).is_some() {
        return true;
    }
    let Some(star_idx) = target.iter().position(is_unpacked_tuple_elem) else {
        return true;
    };
    let (prefix, rest) = target.split_at(star_idx);
    let Some((star_elem, suffix)) = rest.split_first() else {
        return true;
    };
    // Only one unpacked segment is supported.
    if suffix.iter().any(is_unpacked_tuple_elem) {
        return true;
    }
    let InferredType::Named(star_name) = star_elem else {
        return true;
    };

    match parse_star_segment(star_name) {
        StarSegment::Variadic(elem) => {
            let Some(middle_len) = source.len().checked_sub(prefix.len() + suffix.len()) else {
                return false;
            };
            prefix_suffix_match(source, prefix, suffix)
                && match elem {
                    None => true,
                    Some(elem_ty) => source
                        .iter()
                        .skip(prefix.len())
                        .take(middle_len)
                        .all(|s| s.is_assignable_to(&elem_ty)),
                }
        }
        StarSegment::Fixed(middle) => {
            if source.len() != prefix.len() + middle.len() + suffix.len() {
                return false;
            }
            prefix_suffix_match(source, prefix, suffix)
                && source
                    .iter()
                    .skip(prefix.len())
                    .take(middle.len())
                    .zip(middle.iter())
                    .all(|(s, m)| s.is_assignable_to(m))
        }
    }
}

/// Check that a source tuple's leading elements match `prefix` and trailing
/// elements match `suffix` (both fixed, non-starred). Callers guarantee
/// `source.len() >= prefix.len() + suffix.len()`.
fn prefix_suffix_match(
    source: &[InferredType],
    prefix: &[InferredType],
    suffix: &[InferredType],
) -> bool {
    source
        .iter()
        .zip(prefix.iter())
        .all(|(s, p)| s.is_assignable_to(p))
        && source
            .iter()
            .rev()
            .zip(suffix.iter().rev())
            .all(|(s, q)| s.is_assignable_to(q))
}
