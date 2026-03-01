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
        match (a, b) {
            (InferredType::Union(mut types_a), InferredType::Union(types_b)) => {
                types_a.extend(types_b);
                InferredType::Union(types_a)
            }
            (InferredType::Union(mut types), other) => {
                types.push(other);
                InferredType::Union(types)
            }
            (a, InferredType::Union(mut types)) => {
                types.insert(0, a);
                InferredType::Union(types)
            }
            (a, b) => InferredType::Union(vec![a, b]),
        }
    }

    /// Returns true if this type is assignable to the other type.
    #[must_use]
    pub fn is_assignable_to(&self, other: &InferredType) -> bool {
        match (self, other) {
            // Any is assignable to everything
            (_, InferredType::Any) => true,
            // Never is assignable to everything
            (InferredType::Never, _) => true,
            // Same types are assignable
            (a, b) if a == b => true,
            // Int is assignable to float
            (InferredType::Int, InferredType::Float) => true,
            // Literal types are assignable to their base types
            (
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
            // Container types require element type assignability
            (InferredType::List(a), InferredType::List(b)) => a.is_assignable_to(b),
            (InferredType::Dict(a_key, a_val), InferredType::Dict(b_key, b_val)) => {
                a_key.is_assignable_to(b_key) && a_val.is_assignable_to(b_val)
            }
            (InferredType::Set(a), InferredType::Set(b)) => a.is_assignable_to(b),
            (InferredType::Tuple(a), InferredType::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(a_elem, b_elem)| a_elem.is_assignable_to(b_elem))
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        assert_eq!(InferredType::Int.to_string(), "int");
        assert_eq!(InferredType::Str.to_string(), "str");
        assert_eq!(InferredType::Float.to_string(), "float");
        assert_eq!(InferredType::Bool.to_string(), "bool");
        assert_eq!(InferredType::Bytes.to_string(), "bytes");
        assert_eq!(InferredType::None_.to_string(), "None");
        assert_eq!(InferredType::Any.to_string(), "Any");
        assert_eq!(InferredType::Never.to_string(), "Never");
        assert_eq!(InferredType::Unknown.to_string(), "Unknown");
        assert_eq!(InferredType::Named("MyClass".to_string()).to_string(), "MyClass");
        
        assert_eq!(
            InferredType::List(Box::new(InferredType::Int)).to_string(),
            "list[int]"
        );
        assert_eq!(
            InferredType::Dict(Box::new(InferredType::Str), Box::new(InferredType::Int)).to_string(),
            "dict[str, int]"
        );
        assert_eq!(
            InferredType::Set(Box::new(InferredType::Int)).to_string(),
            "set[int]"
        );
        assert_eq!(
            InferredType::Tuple(vec![InferredType::Int, InferredType::Str]).to_string(),
            "tuple[int, str]"
        );
        assert_eq!(
            InferredType::Union(vec![InferredType::Int, InferredType::Str]).to_string(),
            "int | str"
        );
        assert_eq!(
            InferredType::Optional(Box::new(InferredType::Int)).to_string(),
            "Optional[int]"
        );
    }

    #[test]
    fn test_union() {
        let int = InferredType::Int;
        let str = InferredType::Str;
        
        let union1 = InferredType::union(int.clone(), str.clone());
        assert!(matches!(union1, InferredType::Union(ref types) if types.len() == 2));
        
        let float = InferredType::Float;
        let union2 = InferredType::union(union1, float.clone());
        assert!(matches!(union2, InferredType::Union(ref types) if types.len() == 3));
        
        let bool = InferredType::Bool;
        let existing_union = InferredType::Union(vec![bool]);
        let union3 = InferredType::union(existing_union, float.clone());
        assert!(matches!(union3, InferredType::Union(ref types) if types.len() == 2));
    }

    #[test]
    fn test_is_assignable_to() {
        assert!(InferredType::Int.is_assignable_to(&InferredType::Int));
        assert!(InferredType::Int.is_assignable_to(&InferredType::Float));
        assert!(InferredType::Never.is_assignable_to(&InferredType::Int));
        assert!(InferredType::Int.is_assignable_to(&InferredType::Any));
        
        assert!(InferredType::Optional(Box::new(InferredType::Int))
            .is_assignable_to(&InferredType::Optional(Box::new(InferredType::Int))));
        assert!(InferredType::Int.is_assignable_to(&InferredType::Optional(Box::new(InferredType::Int))));
        
        let union = InferredType::Union(vec![InferredType::Int, InferredType::Str]);
        assert!(union.is_assignable_to(&InferredType::Any));
        assert!(InferredType::Int.is_assignable_to(&union));
    }
}