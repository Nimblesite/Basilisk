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
            (InferredType::Int, InferredType::Float)
            | (
                InferredType::Literal(_),
                InferredType::Int | InferredType::Str | InferredType::Float | InferredType::Bool,
            )
            | (
                InferredType::Str | InferredType::Literal(LiteralValue::Str(_)),
                InferredType::LiteralString,
            )
            | (InferredType::LiteralString, InferredType::Str)
            // None is always assignable to Optional[T]
            | (InferredType::None_, InferredType::Optional(_))
            // Named types may be callable (classes, protocols, type objects).
            // Without full signature resolution we cannot verify — assume
            // compatible to avoid false positives.
            | (InferredType::Named(_), InferredType::Callable(_))
            | (InferredType::Callable(_), InferredType::Named(_)) => true,
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
                // Variable-length tuple: tuple[T, ...] is Tuple([T, Named("...")])
                if let Some(b_elem) = var_length_tuple_element(b) {
                    return a.iter().all(|e| e.is_assignable_to(b_elem));
                }
                if let Some(a_elem) = var_length_tuple_element(a) {
                    // tuple[Any, ...] is bidirectionally compatible with any tuple
                    if matches!(a_elem, InferredType::Any) {
                        return true;
                    }
                    // Variable-length source cannot satisfy fixed-length target.
                    return false;
                }
                // If lengths differ and either contains Named elements (e.g.
                // unpacked *tuple[T, ...] syntax), assume compatible — we
                // cannot verify structural compatibility without expansion.
                if a.len() != b.len() {
                    let has_named = a
                        .iter()
                        .chain(b.iter())
                        .any(|e| matches!(e, InferredType::Named(_)));
                    return has_named;
                }
                a.iter()
                    .zip(b.iter())
                    .all(|(a_elem, b_elem)| a_elem.is_assignable_to(b_elem))
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
            // Concrete type to Named: check builtin MRO (e.g. None_ to Named("hashable")).
            (concrete, InferredType::Named(target_name)) => {
                if let Some(source_name) = concrete.builtin_type_name() {
                    if let Some(mro) = crate::subtyping::builtin_mro(source_name) {
                        return mro
                            .iter()
                            .any(|m| m.eq_ignore_ascii_case(target_name));
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Map this type to its Python builtin type name for MRO lookups.
    #[must_use]
    fn builtin_type_name(&self) -> Option<&'static str> {
        match self {
            InferredType::Int => Some("int"),
            InferredType::Str => Some("str"),
            InferredType::Float => Some("float"),
            InferredType::Bool => Some("bool"),
            InferredType::Bytes => Some("bytes"),
            InferredType::None_ => Some("NoneType"),
            InferredType::List(_) => Some("list"),
            InferredType::Dict(_, _) => Some("dict"),
            InferredType::Set(_) => Some("set"),
            InferredType::Tuple(_) => Some("tuple"),
            _ => None,
        }
    }
}

/// Returns the element type if `elems` represents a variable-length
/// tuple `tuple[T, ...]` (parsed as `Tuple([T, Named("...")])`.
pub(crate) fn var_length_tuple_element(elems: &[InferredType]) -> Option<&InferredType> {
    if let [first, InferredType::Named(name)] = elems {
        if name == "..." {
            return Some(first);
        }
    }
    None
}
