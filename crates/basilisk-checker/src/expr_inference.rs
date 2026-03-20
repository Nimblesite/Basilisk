//! Expression type inference engine.
//!
//! Extends the basic `infer_rhs()` (literal-only) with full expression
//! inference: function call return types, attribute access, subscripts,
//! binary/unary ops, conditionals, and walrus operators.
//!
//! See `CHECKER-TYPE-INFERENCE-SPEC.md` §2.3, §8.

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;

use crate::types::InferredType;

/// Infers expression types using module-level context.
///
/// Unlike `infer_rhs()` which only handles literals, this engine can
/// resolve function call return types, constructor calls, attribute
/// access, and more by consulting the `ResolvedModule`.
pub struct ExpressionInferrer<'a> {
    /// The resolved module providing function/class definitions.
    module: &'a ResolvedModule,
    /// Function name → return type annotation text (cached).
    func_return_types: HashMap<&'a str, &'a str>,
    /// Class name → `ClassInfo` index (cached).
    class_names: HashMap<&'a str, usize>,
}

impl<'a> ExpressionInferrer<'a> {
    /// Build an inferrer from a resolved module.
    #[must_use]
    pub fn new(module: &'a ResolvedModule) -> Self {
        let mut func_return_types = HashMap::new();
        for func in &module.functions {
            if let Some(span) = &func.return_annotation_span {
                if let Some(text) = span.slice_source(&module.source) {
                    let _ = func_return_types.insert(func.name.as_str(), text);
                }
            }
        }

        let mut class_names = HashMap::new();
        for (idx, cls) in module.classes.iter().enumerate() {
            let _ = class_names.insert(cls.name.as_str(), idx);
        }

        Self {
            module,
            func_return_types,
            class_names,
        }
    }

    // -----------------------------------------------------------------
    // 2a. Function call return type resolution (same-module)
    // -----------------------------------------------------------------

    /// Resolve the return type of a function call by name.
    ///
    /// Looks up the function in the current module's definitions and
    /// returns the annotated return type if available.
    #[must_use]
    pub fn resolve_call_return_type(&self, callee_name: &str) -> Option<InferredType> {
        // Check same-module functions
        if let Some(&return_text) = self.func_return_types.get(callee_name) {
            return Some(InferredType::from_annotation(return_text));
        }

        // 2b. Constructor call: `ClassName()` → Named(ClassName)
        if self.class_names.contains_key(callee_name) {
            return Some(InferredType::Named(callee_name.to_owned()));
        }

        // 2c. Cross-module: check imported symbols
        if let Some(ext) = self.module.imported_symbols.get(callee_name) {
            if let Some(ref ann) = ext.type_annotation {
                return Some(InferredType::from_annotation(ann));
            }
            // Imported class used as constructor
            if ext.kind == basilisk_resolver::scope::ExternalSymbolKind::Class {
                return Some(InferredType::Named(callee_name.to_owned()));
            }
        }

        // Builtin constructors
        resolve_builtin_constructor(callee_name)
    }

    // -----------------------------------------------------------------
    // 2d. Method call resolution (obj.method())
    // -----------------------------------------------------------------

    /// Resolve the return type of a method call `obj.method()`.
    ///
    /// Requires knowing the type of `obj` and looking up `method` on
    /// its class definition.
    #[must_use]
    pub fn resolve_method_return_type(
        &self,
        obj_type: &InferredType,
        method_name: &str,
    ) -> Option<InferredType> {
        let class_name = type_to_class_name(obj_type)?;

        // Check same-module classes
        if let Some(&cls_idx) = self.class_names.get(class_name) {
            let cls = self.module.classes.get(cls_idx)?;
            // Find the method in module functions that belong to this class
            for func in &self.module.functions {
                if func.class_name.as_deref() == Some(&cls.name) && func.name == method_name {
                    if let Some(span) = &func.return_annotation_span {
                        if let Some(text) = span.slice_source(&self.module.source) {
                            return Some(InferredType::from_annotation(text));
                        }
                    }
                }
            }
        }

        // Builtin method return types
        resolve_builtin_method(class_name, method_name)
    }

    // -----------------------------------------------------------------
    // 2e. Attribute access type resolution
    // -----------------------------------------------------------------

    /// Resolve the type of `obj.attr`.
    #[must_use]
    pub fn resolve_attribute_type(
        &self,
        obj_type: &InferredType,
        attr_name: &str,
    ) -> Option<InferredType> {
        let class_name = type_to_class_name(obj_type)?;

        if let Some(&cls_idx) = self.class_names.get(class_name) {
            let cls = self.module.classes.get(cls_idx)?;
            for attr in &cls.attributes {
                if attr.name == attr_name {
                    if let Some(ref span) = attr.annotation_span {
                        if let Some(text) = span.slice_source(&self.module.source) {
                            return Some(InferredType::from_annotation(text));
                        }
                    }
                }
            }
        }

        None
    }

    // -----------------------------------------------------------------
    // 2f. Subscript type resolution
    // -----------------------------------------------------------------

    /// Resolve the type of `obj[key]`.
    #[must_use]
    pub fn resolve_subscript_type(
        &self,
        obj_type: &InferredType,
        _key_type: &InferredType,
    ) -> Option<InferredType> {
        match obj_type {
            InferredType::List(elem) => Some(elem.as_ref().clone()),
            InferredType::Dict(_, val) => Some(val.as_ref().clone()),
            InferredType::Tuple(elems) => {
                // For tuple with literal int key, return the specific element type
                // For now, return union of all element types
                if elems.is_empty() {
                    None
                } else if elems.len() == 1 {
                    elems.first().cloned()
                } else {
                    Some(
                        elems
                            .iter()
                            .cloned()
                            .reduce(InferredType::union)
                            .unwrap_or(InferredType::Unknown),
                    )
                }
            }
            InferredType::Str => Some(InferredType::Str),
            InferredType::Bytes => Some(InferredType::Int),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// 2g. Binary/unary operation return types (builtin table)
// ---------------------------------------------------------------------------

/// Infer the return type of a binary operation.
#[must_use]
pub fn infer_binop_type(left: &InferredType, right: &InferredType, op: BinOp) -> InferredType {
    match op {
        // Arithmetic: +, -, *, //, %, **
        BinOp::Add | BinOp::Sub | BinOp::Mult | BinOp::FloorDiv | BinOp::Mod | BinOp::Pow => {
            match (left, right) {
                (InferredType::Int, InferredType::Int) => InferredType::Int,
                (InferredType::Float, InferredType::Int | InferredType::Float)
                | (InferredType::Int, InferredType::Float) => InferredType::Float,
                (InferredType::Str, InferredType::Str) if matches!(op, BinOp::Add) => {
                    InferredType::Str
                }
                (InferredType::List(a), InferredType::List(b)) if matches!(op, BinOp::Add) => {
                    InferredType::List(Box::new(InferredType::union(
                        a.as_ref().clone(),
                        b.as_ref().clone(),
                    )))
                }
                (InferredType::Str, InferredType::Int) | (InferredType::Int, InferredType::Str)
                    if matches!(op, BinOp::Mult) =>
                {
                    InferredType::Str
                }
                _ => InferredType::Unknown,
            }
        }
        // True division always returns float
        BinOp::Div => InferredType::Float,
        // Bitwise ops on ints return int
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::LShift | BinOp::RShift => {
            match (left, right) {
                (
                    InferredType::Int | InferredType::Bool,
                    InferredType::Int | InferredType::Bool,
                ) => InferredType::Int,
                (InferredType::Set(a), InferredType::Set(b))
                    if matches!(op, BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor) =>
                {
                    InferredType::Set(Box::new(InferredType::union(
                        a.as_ref().clone(),
                        b.as_ref().clone(),
                    )))
                }
                _ => InferredType::Unknown,
            }
        }
        // Matmul — unknown without type info
        BinOp::MatMult => InferredType::Unknown,
    }
}

/// Infer the return type of a unary operation.
#[must_use]
pub fn infer_unaryop_type(operand: &InferredType, op: UnaryOp) -> InferredType {
    match op {
        UnaryOp::Not => InferredType::Bool,
        UnaryOp::Neg => match operand {
            InferredType::Int | InferredType::Bool => InferredType::Int,
            InferredType::Float => InferredType::Float,
            _ => InferredType::Unknown,
        },
        UnaryOp::Pos => match operand {
            InferredType::Int | InferredType::Float | InferredType::Bool => operand.clone(),
            _ => InferredType::Unknown,
        },
        UnaryOp::Invert => match operand {
            InferredType::Int | InferredType::Bool => InferredType::Int,
            _ => InferredType::Unknown,
        },
    }
}

// ---------------------------------------------------------------------------
// 2h. Conditional expression (a if cond else b → union)
// ---------------------------------------------------------------------------

/// Infer the type of `true_val if cond else false_val`.
#[must_use]
pub fn infer_conditional_type(true_type: &InferredType, false_type: &InferredType) -> InferredType {
    InferredType::union(true_type.clone(), false_type.clone())
}

// ---------------------------------------------------------------------------
// 2i. Walrus operator — type is same as the expression
// (No separate function needed — the caller passes through the expr type)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Comparison operators always return bool
// ---------------------------------------------------------------------------

/// Infer the return type of a comparison operation.
#[must_use]
pub fn infer_compare_type() -> InferredType {
    InferredType::Bool
}

// ---------------------------------------------------------------------------
// Binary/unary op enums (subset of Python ops relevant to type inference)
// ---------------------------------------------------------------------------

/// Binary operator kinds for type inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mult,
    /// `@`
    MatMult,
    /// `/`
    Div,
    /// `//`
    FloorDiv,
    /// `%`
    Mod,
    /// `**`
    Pow,
    /// `&`
    BitAnd,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `<<`
    LShift,
    /// `>>`
    RShift,
}

/// Unary operator kinds for type inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `not`
    Not,
    /// `-`
    Neg,
    /// `+`
    Pos,
    /// `~`
    Invert,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the class name from an `InferredType`, if it represents a named/known class.
fn type_to_class_name(ty: &InferredType) -> Option<&str> {
    match ty {
        InferredType::Named(name) => {
            // Strip generic params: "Foo[int]" → "Foo"
            Some(name.split('[').next().unwrap_or(name))
        }
        InferredType::Int => Some("int"),
        InferredType::Str => Some("str"),
        InferredType::Float => Some("float"),
        InferredType::Bool => Some("bool"),
        InferredType::Bytes => Some("bytes"),
        InferredType::List(_) => Some("list"),
        InferredType::Dict(_, _) => Some("dict"),
        InferredType::Set(_) => Some("set"),
        InferredType::Tuple(_) => Some("tuple"),
        _ => None,
    }
}

/// Resolve builtin constructor return types.
fn resolve_builtin_constructor(name: &str) -> Option<InferredType> {
    match name {
        "int" | "len" | "abs" | "hash" | "id" | "ord" => Some(InferredType::Int),
        "str" | "chr" | "repr" | "hex" | "oct" | "bin" | "format" | "ascii" | "input" => {
            Some(InferredType::Str)
        }
        "float" => Some(InferredType::Float),
        "bool" | "isinstance" | "issubclass" | "callable" | "hasattr" => Some(InferredType::Bool),
        "bytes" | "bytearray" => Some(InferredType::Bytes),
        "list" | "sorted" => Some(InferredType::List(Box::new(InferredType::Any))),
        "dict" => Some(InferredType::Dict(
            Box::new(InferredType::Any),
            Box::new(InferredType::Any),
        )),
        "set" | "frozenset" => Some(InferredType::Set(Box::new(InferredType::Any))),
        "tuple" => Some(InferredType::Tuple(Vec::new())),
        "object" | "type" | "super" | "range" | "enumerate" | "zip" | "map" | "filter"
        | "reversed" => Some(InferredType::Named(name.to_owned())),
        "max" | "min" | "sum" | "round" | "pow" | "divmod" | "next" => Some(InferredType::Unknown),
        "print" | "setattr" | "delattr" => Some(InferredType::None_),
        "iter" => Some(InferredType::Named("Iterator".to_owned())),
        "open" => Some(InferredType::Named("TextIOWrapper".to_owned())),
        "getattr" => Some(InferredType::Any),
        "vars" => Some(InferredType::Dict(
            Box::new(InferredType::Str),
            Box::new(InferredType::Any),
        )),
        "dir" => Some(InferredType::List(Box::new(InferredType::Str))),
        _ => None,
    }
}

/// Resolve builtin method return types for common types.
fn resolve_builtin_method(class_name: &str, method: &str) -> Option<InferredType> {
    match (class_name, method) {
        // → Str
        (
            "str",
            "upper" | "lower" | "strip" | "lstrip" | "rstrip" | "title" | "capitalize" | "casefold"
            | "swapcase" | "center" | "ljust" | "rjust" | "zfill" | "replace" | "join" | "format"
            | "removeprefix" | "removesuffix" | "expandtabs",
        )
        | ("bytes", "decode")
        | ("float", "hex") => Some(InferredType::Str),

        // → Int
        ("str", "find" | "rfind" | "index" | "rindex" | "count")
        | ("list" | "tuple", "index" | "count")
        | ("int", "bit_length" | "bit_count") => Some(InferredType::Int),

        // → Bool
        (
            "str",
            "startswith" | "endswith" | "isalpha" | "isdigit" | "isalnum" | "isspace" | "isupper"
            | "islower" | "istitle" | "isascii" | "isdecimal" | "isnumeric" | "isidentifier"
            | "isprintable",
        )
        | ("set", "issubset" | "issuperset" | "isdisjoint")
        | ("float", "is_integer") => Some(InferredType::Bool),

        // → Bytes
        ("str", "encode") | ("int", "to_bytes") => Some(InferredType::Bytes),

        // → None
        ("list", "append" | "extend" | "insert" | "remove" | "clear" | "sort" | "reverse")
        | ("dict", "update" | "clear")
        | ("set", "add" | "remove" | "discard" | "clear" | "update") => Some(InferredType::None_),

        // → Unknown
        ("list" | "set", "pop") | ("dict", "get" | "pop" | "setdefault") => {
            Some(InferredType::Unknown)
        }

        // → List[str]
        ("str", "split" | "rsplit" | "splitlines") => {
            Some(InferredType::List(Box::new(InferredType::Str)))
        }

        // → Tuple[str, str, str]
        ("str", "partition" | "rpartition") => Some(InferredType::Tuple(vec![
            InferredType::Str,
            InferredType::Str,
            InferredType::Str,
        ])),

        // → Tuple[int, int]
        ("int" | "float", "as_integer_ratio") => Some(InferredType::Tuple(vec![
            InferredType::Int,
            InferredType::Int,
        ])),

        // → Dict/Set/List (copy, etc.)
        ("str", "maketrans") => Some(InferredType::Dict(
            Box::new(InferredType::Int),
            Box::new(InferredType::Any),
        )),
        ("list", "copy") => Some(InferredType::List(Box::new(InferredType::Any))),
        ("dict", "copy") => Some(InferredType::Dict(
            Box::new(InferredType::Any),
            Box::new(InferredType::Any),
        )),
        ("set", "copy" | "union" | "intersection" | "difference" | "symmetric_difference") => {
            Some(InferredType::Set(Box::new(InferredType::Any)))
        }

        // → Named types
        ("dict", "keys") => Some(InferredType::Named("dict_keys".to_owned())),
        ("dict", "values") => Some(InferredType::Named("dict_values".to_owned())),
        ("dict", "items") => Some(InferredType::Named("dict_items".to_owned())),

        _ => None,
    }
}

/// Resolve the return type of a callable type.
#[must_use]
pub fn callable_return_type(ty: &InferredType) -> Option<InferredType> {
    match ty {
        InferredType::Callable(info) => Some(info.return_type.as_ref().clone()),
        _ => None,
    }
}
