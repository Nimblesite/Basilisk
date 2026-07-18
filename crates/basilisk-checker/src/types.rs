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
    /// Any type (`Any`) - explicit escape hatch.
    /// Implements [TYPEINF-SPECIAL-ANY] — the explicit escape hatch variant; never
    /// inferred as a fallback (unannotated params produce E0001, not `Any`).
    Any,
    /// Never type (`Never`) - bottom type, no values.
    /// Implements [TYPEINF-SPECIAL-NEVER] — bottom type used for always-raises
    /// return inference and pattern-match exhaustiveness.
    Never,
    /// Unknown type - used when type cannot be determined
    Unknown,
    /// `LiteralString` — any string literal or value known to be a literal string.
    /// Implements [TYPEINF-SPECIAL-LITERALSTRING] — supertype of all `Literal[str]`.
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
    ///
    /// Implements [TYPEINF-VARS-FLOW] — the join of types assigned across branches
    /// is their union. Also the `A | B` construction underpinning
    /// [TYPEINF-SUBTYPING-UNION]; `Never ∪ T = T` realises `Never` as the bottom
    /// type (identity element of union).
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
    ///
    /// Implements [TYPEINF-SUBTYPING-IMPL] — the `InferredType`-level subtype check
    /// the spec calls `is_subtype_of()`. Covers the builtin numeric tower
    /// ([TYPEINF-SUBTYPING-NOMINAL]), union/optional/special-form relations
    /// ([TYPEINF-SUBTYPING-UNION]), container element assignability
    /// ([TYPEINF-SUBTYPING-GENERIC]) and callable variance
    /// ([TYPEINF-SUBTYPING-CALLABLE]). Nominal MRO and structural Protocol/TypedDict
    /// checks live in out-of-scope rule modules (see the consolidated map).
    #[must_use]
    pub fn is_assignable_to(&self, other: &InferredType) -> bool {
        match (self, other) {
            // Any is assignable to/from everything (PEP 484).
            // Unknown means we cannot determine the type — assume compatible to avoid false positives.
            // Never is the bottom type — assignable to everything.
            // Implements [TYPEINF-SPECIAL-ANY] (Any bidirectional) and
            // [TYPEINF-SPECIAL-NEVER] (Never <: everything, bottom type).
            (InferredType::Any | InferredType::Never | InferredType::Unknown, _)
            | (_, InferredType::Any | InferredType::Unknown) => true,
            // Same types are assignable
            (a, b) if a == b => true,
            // Int→float widening, Literal assignable to base type,
            // string types assignable to LiteralString, LiteralString to str.
            // Implements [TYPEINF-SUBTYPING-NOMINAL] — the hardcoded builtin tower
            // (bool <: int <: float) plus [TYPEINF-SPECIAL-LITERALSTRING]
            // (Literal[str]/LiteralString <-> str relations).
            (
                InferredType::Int | InferredType::Literal(LiteralValue::Float(_)),
                InferredType::Float,
            )
            // `bool` is a subclass of `int` (PEP 484), so plain `bool` widens
            // through the whole tower exactly like `Literal[bool]` does below.
            | (
                InferredType::Literal(LiteralValue::Int(_)) | InferredType::Bool,
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
            // Union on the LEFT decomposes before Optional-target unwrapping:
            // `A | None <: Optional[B]` must check each variant against the
            // whole `Optional[B]` (so the `None` arm can satisfy it), not
            // against the unwrapped `B`.
            // Implements [TYPEINF-SUBTYPING-UNION] — `A | B <: C` iff `A <: C` and
            // `B <: C`.
            (InferredType::Union(types), other) => types.iter().all(|t| t.is_assignable_to(other)),
            // Optional types are assignable to their non-optional counterparts.
            // Implements [TYPEINF-SUBTYPING-UNION] — Optional[T] = T | None handling.
            (InferredType::Optional(inner), other) => inner.is_assignable_to(other),
            (inner, InferredType::Optional(other)) => inner.is_assignable_to(other),
            // `A <: A | B` (a type is a subtype of any union containing it).
            (inner, InferredType::Union(types)) => types.iter().any(|t| inner.is_assignable_to(t)),
            // Container types require element type assignability.
            // Implements [TYPEINF-SUBTYPING-GENERIC] — list/set/dict are invariant
            // here (element types checked structurally, no cross-container matching).
            // List and Set cannot use or-patterns — that would incorrectly allow cross-matching.
            (InferredType::List(a), InferredType::List(b))
            | (InferredType::Set(a), InferredType::Set(b)) => a.is_assignable_to(b),
            (InferredType::Dict(a_key, a_val), InferredType::Dict(b_key, b_val)) => {
                a_key.is_assignable_to(b_key) && a_val.is_assignable_to(b_val)
            }
            (InferredType::Tuple(a), InferredType::Tuple(b)) => {
                // Implements [TYPEINF-COLLECTIONS-TUPLES] — fixed-length positional
                // tuples plus the homogeneous `tuple[X, ...]` and PEP 646 unpacked
                // (`*tuple[...]`/`*Ts`) forms.
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
            // Callable type assignability.
            // Implements [TYPEINF-SUBTYPING-CALLABLE] — return type covariant
            // (source return <: target return), parameters contravariant
            // (target param <: source param), `...`/empty params gradual.
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

pub(crate) use crate::types_star_tuples::{
    homogeneous_tuple_elem, is_unpacked_tuple_elem, tuple_assignable_with_star,
};
