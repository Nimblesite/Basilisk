//! Type representation for Basilisk's type inference engine.

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
    /// Any type (`Any`) - explicit escape hatch
    Any,
    /// Never type (`Never`) - bottom type, no values
    Never,
    /// Unknown type - used when type cannot be determined
    Unknown,
    /// Named type (`ClassName`) - fallback for named types not yet resolved
    Named(String),
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
            InferredType::Any => write!(f, "Any"),
            InferredType::Never => write!(f, "Never"),
            InferredType::Unknown => write!(f, "Unknown"),
            InferredType::Named(name) => write!(f, "{name}"),
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
        // Handle Never specially: Never ∪ T = T
        if matches!(a, InferredType::Never) {
            return b;
        }
        if matches!(b, InferredType::Never) {
            return a;
        }
        
        // Flatten both sides into vectors of types
        let mut types = Vec::new();
        
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
            // Any target or Never source is always assignable
            (_, InferredType::Any) | (InferredType::Never, _) => true,
            // Same types are assignable
            (a, b) if a == b => true,
            // Int→float widening, or Literal assignable to its base type
            (InferredType::Int, InferredType::Float)
            | (
                InferredType::Literal(_),
                InferredType::Int | InferredType::Str | InferredType::Float | InferredType::Bool,
            ) => true,
            // Optional types are assignable to their non-optional counterparts
            (InferredType::Optional(inner), other) => inner.is_assignable_to(other),
            (inner, InferredType::Optional(other)) => inner.is_assignable_to(other),
            // Union types require all variants to be assignable
            (InferredType::Union(types), other) => {
                types.iter().all(|t| t.is_assignable_to(other))
            }
            (inner, InferredType::Union(types)) => {
                types.iter().any(|t| inner.is_assignable_to(t))
            }
            // Container types require element type assignability.
            // List and Set cannot use or-patterns — that would incorrectly allow cross-matching.
            (InferredType::List(a), InferredType::List(b)) | (InferredType::Set(a), InferredType::Set(b)) => a.is_assignable_to(b),
            (InferredType::Dict(a_key, a_val), InferredType::Dict(b_key, b_val)) => {
                a_key.is_assignable_to(b_key) && a_val.is_assignable_to(b_val)
            }
            (InferredType::Tuple(a), InferredType::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(a_elem, b_elem)| a_elem.is_assignable_to(b_elem))
            }
            _ => false,
        }
    }

    /// Parses annotation text into an `InferredType`.
    /// 
    /// This is a simplified parser that handles basic type annotations.
    /// For complex types, it returns Named(String) as a fallback.
    #[must_use]
    pub fn from_annotation(annotation: &str) -> InferredType {
        let annotation = annotation.trim().to_ascii_lowercase();
        
        match annotation.as_str() {
            "int" => InferredType::Int,
            "str" => InferredType::Str,
            "float" => InferredType::Float,
            "bool" => InferredType::Bool,
            "bytes" => InferredType::Bytes,
            "none" => InferredType::None_,
            "any" | "object" => InferredType::Any,
            "never" => InferredType::Never,
            _ => {
                // Handle simple container types
                if annotation.starts_with("list[") && annotation.ends_with(']') {
                    let inner = &annotation[5..annotation.len()-1];
                    InferredType::List(Box::new(InferredType::from_annotation(inner)))
                } else if annotation.starts_with("dict[") && annotation.ends_with(']') {
                    let inner = &annotation[5..annotation.len()-1];
                    let parts: Vec<&str> = inner.split(',').collect();
                    if parts.len() == 2 {
                        let key_type = InferredType::from_annotation(parts[0].trim());
                        let value_type = InferredType::from_annotation(parts[1].trim());
                        InferredType::Dict(Box::new(key_type), Box::new(value_type))
                    } else {
                        InferredType::Named(annotation)
                    }
                } else if annotation.starts_with("set[") && annotation.ends_with(']') {
                    let inner = &annotation[4..annotation.len()-1];
                    InferredType::Set(Box::new(InferredType::from_annotation(inner)))
                } else if annotation.starts_with("tuple[") && annotation.ends_with(']') {
                    let inner = &annotation[6..annotation.len()-1];
                    let parts: Vec<&str> = inner.split(',').collect();
                    let elem_types: Vec<InferredType> = parts
                        .iter()
                        .map(|part| InferredType::from_annotation(part.trim()))
                        .collect();
                    InferredType::Tuple(elem_types)
                } else if annotation.starts_with("optional[") && annotation.ends_with(']') {
                    let inner = &annotation[9..annotation.len()-1];
                    InferredType::Optional(Box::new(InferredType::from_annotation(inner)))
                } else if annotation.contains('|') {
                    let parts: Vec<&str> = annotation.split('|').collect();
                    let types: Vec<InferredType> = parts
                        .iter()
                        .map(|part| InferredType::from_annotation(part.trim()))
                        .collect();
                    InferredType::Union(types)
                } else {
                    // Fallback for unknown types
                    InferredType::Named(annotation)
                }
            }
        }
    }
}

