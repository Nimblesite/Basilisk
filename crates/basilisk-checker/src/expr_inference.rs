//! Expression-level type inference engine.
//!
//! Resolves the type of complex expressions: function calls (same-module
//! and cross-module), constructor calls, method calls, attribute access,
//! subscript operations, binary/unary operators, and conditional expressions.

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule};

use crate::span_util::slice_span;
use crate::types::{CallableInfo, InferredType};

/// Expression inferrer that resolves types of complex expressions using
/// module-level metadata from the resolver.
pub struct ExpressionInferrer<'a> {
    /// Function name -> return type annotation text.
    function_returns: HashMap<&'a str, InferredType>,
    /// Class name -> constructor return type (the class itself).
    class_names: HashMap<&'a str, InferredType>,
    /// (`class_name`, `method_name`) -> return type.
    method_returns: HashMap<(&'a str, &'a str), InferredType>,
    /// (`class_name`, `attr_name`) -> attribute type.
    attr_types: HashMap<(&'a str, &'a str), InferredType>,
    /// Imported symbol name -> type annotation from external module.
    imported_types: HashMap<&'a str, InferredType>,
}

/// Table of builtin function return types.
fn builtin_return_type(name: &str) -> Option<InferredType> {
    match name {
        "int" | "len" | "hash" | "id" | "ord" | "round" => Some(InferredType::Int),
        "str" | "repr" | "chr" | "hex" | "oct" | "bin" | "input" => Some(InferredType::Str),
        "float" => Some(InferredType::Float),
        "bool" | "isinstance" | "issubclass" | "hasattr" | "callable" | "any" | "all" => {
            Some(InferredType::Bool)
        }
        "bytes" => Some(InferredType::Bytes),
        "print" | "setattr" | "delattr" => Some(InferredType::None_),
        "abs" | "next" | "min" | "max" | "sum" | "pow" | "getattr" | "super" | "staticmethod"
        | "classmethod" | "property" => Some(InferredType::Unknown),
        "complex" => Some(InferredType::Named("complex".to_owned())),
        "frozenset" => Some(InferredType::Named("frozenset".to_owned())),
        "type" => Some(InferredType::Named("type".to_owned())),
        "open" => Some(InferredType::Named("TextIOWrapper".to_owned())),
        "reversed" => Some(InferredType::Named("reversed".to_owned())),
        "enumerate" => Some(InferredType::Named("enumerate".to_owned())),
        "zip" => Some(InferredType::Named("zip".to_owned())),
        "map" => Some(InferredType::Named("map".to_owned())),
        "filter" => Some(InferredType::Named("filter".to_owned())),
        "range" => Some(InferredType::Named("range".to_owned())),
        "iter" => Some(InferredType::Named("Iterator".to_owned())),
        "list" | "sorted" => Some(InferredType::List(Box::new(InferredType::Any))),
        "dir" => Some(InferredType::List(Box::new(InferredType::Str))),
        "dict" => Some(InferredType::Dict(
            Box::new(InferredType::Any),
            Box::new(InferredType::Any),
        )),
        "vars" => Some(InferredType::Dict(
            Box::new(InferredType::Str),
            Box::new(InferredType::Any),
        )),
        "set" => Some(InferredType::Set(Box::new(InferredType::Any))),
        "tuple" => Some(InferredType::Tuple(vec![])),
        "divmod" => Some(InferredType::Tuple(vec![
            InferredType::Int,
            InferredType::Int,
        ])),
        _ => None,
    }
}

/// Table of builtin method return types for common types.
///
/// Arms are grouped by return type to satisfy `clippy::match_same_arms`.
fn builtin_method_return_type(type_name: &str, method_name: &str) -> Option<InferredType> {
    match (type_name, method_name) {
        // Methods returning Str
        (
            "str",
            "upper" | "lower" | "strip" | "lstrip" | "rstrip" | "title" | "capitalize" | "casefold"
            | "swapcase" | "center" | "ljust" | "rjust" | "zfill" | "replace" | "removeprefix"
            | "removesuffix" | "join" | "format" | "format_map",
        )
        | ("float", "hex")
        | ("bytes", "decode" | "hex") => Some(InferredType::Str),

        // Methods returning Int
        ("str" | "bytes", "find" | "rfind" | "index" | "rindex" | "count")
        | ("list", "count" | "index")
        | ("int", "bit_length" | "bit_count" | "conjugate") => Some(InferredType::Int),

        // Methods returning Bool
        (
            "str",
            "startswith" | "endswith" | "isalpha" | "isdigit" | "isalnum" | "isspace" | "islower"
            | "isupper" | "istitle" | "isidentifier" | "isprintable" | "isnumeric" | "isdecimal"
            | "isascii",
        )
        | ("set", "issubset" | "issuperset" | "isdisjoint")
        | ("float", "is_integer")
        | ("bytes", "startswith" | "endswith") => Some(InferredType::Bool),

        // Methods returning Bytes
        ("str", "encode") | ("int", "to_bytes") => Some(InferredType::Bytes),

        // Methods returning None (mutating in-place)
        ("list", "append" | "extend" | "insert" | "remove" | "clear" | "reverse" | "sort")
        | ("dict", "update" | "clear")
        | (
            "set",
            "add"
            | "discard"
            | "remove"
            | "clear"
            | "update"
            | "intersection_update"
            | "difference_update"
            | "symmetric_difference_update",
        ) => Some(InferredType::None_),

        // Methods returning Unknown (type depends on container contents)
        ("list" | "set", "pop") | ("dict", "get" | "pop" | "setdefault") => {
            Some(InferredType::Unknown)
        }

        // Methods with unique return types
        ("str", "split" | "rsplit" | "splitlines") => {
            Some(InferredType::List(Box::new(InferredType::Str)))
        }
        ("str", "partition" | "rpartition") => Some(InferredType::Tuple(vec![
            InferredType::Str,
            InferredType::Str,
            InferredType::Str,
        ])),
        ("str", "maketrans") => Some(InferredType::Dict(
            Box::new(InferredType::Int),
            Box::new(InferredType::Any),
        )),
        ("list", "copy") => Some(InferredType::List(Box::new(InferredType::Any))),
        ("dict", "keys") => Some(InferredType::Named("dict_keys".to_owned())),
        ("dict", "values") => Some(InferredType::Named("dict_values".to_owned())),
        ("dict", "items") => Some(InferredType::Named("dict_items".to_owned())),
        ("dict", "copy") => Some(InferredType::Dict(
            Box::new(InferredType::Any),
            Box::new(InferredType::Any),
        )),
        ("dict", "popitem") => Some(InferredType::Tuple(vec![
            InferredType::Unknown,
            InferredType::Unknown,
        ])),
        ("set", "copy" | "union" | "intersection" | "difference" | "symmetric_difference") => {
            Some(InferredType::Set(Box::new(InferredType::Any)))
        }
        ("float", "as_integer_ratio") => Some(InferredType::Tuple(vec![
            InferredType::Int,
            InferredType::Int,
        ])),
        ("float", "conjugate") => Some(InferredType::Float),
        ("bytes", "split" | "rsplit" | "splitlines") => {
            Some(InferredType::List(Box::new(InferredType::Bytes)))
        }

        _ => None,
    }
}

impl<'a> ExpressionInferrer<'a> {
    /// Build an expression inferrer from a resolved module.
    #[must_use]
    pub fn from_module(module: &'a ResolvedModule) -> Self {
        let mut function_returns = HashMap::new();
        let mut class_names = HashMap::new();
        let mut method_returns = HashMap::new();
        let mut attr_types = HashMap::new();
        let mut imported_types = HashMap::new();

        // Index function return types.
        for func in &module.functions {
            if func.class_name.is_some() {
                // Method — index under (class_name, method_name).
                if let Some(class_name) = &func.class_name {
                    if let Some(ret_type) = Self::resolve_return_type(func, &module.source) {
                        let _ = method_returns
                            .insert((class_name.as_str(), func.name.as_str()), ret_type);
                    }
                }
            } else if let Some(ret_type) = Self::resolve_return_type(func, &module.source) {
                let _ = function_returns.insert(func.name.as_str(), ret_type);
            }
        }

        // Index class names as constructors.
        for class in &module.classes {
            let _ =
                class_names.insert(class.name.as_str(), InferredType::Named(class.name.clone()));
        }

        // Index class attributes.
        for class in &module.classes {
            for attr in &class.attributes {
                if let Some(ann_span) = attr.annotation_span {
                    if let Some(ann_text) = slice_span(&module.source, ann_span) {
                        let inferred = InferredType::from_annotation(ann_text.trim());
                        let _ =
                            attr_types.insert((class.name.as_str(), attr.name.as_str()), inferred);
                    }
                }
            }
        }

        // Index imported symbols.
        for (name, symbol) in &module.imported_symbols {
            if let Some(ref type_ann) = symbol.type_annotation {
                let inferred = InferredType::from_annotation(type_ann);
                let _ = imported_types.insert(name.as_str(), inferred);
            }
        }

        Self {
            function_returns,
            class_names,
            method_returns,
            attr_types,
            imported_types,
        }
    }

    /// Resolve a function's return type from its annotation.
    fn resolve_return_type(func: &FunctionInfo, source: &str) -> Option<InferredType> {
        let ann_span = func.return_annotation_span?;
        let ann_text = slice_span(source, ann_span)?;
        Some(InferredType::from_annotation(ann_text.trim()))
    }

    /// Resolve the return type of a function call by name.
    #[must_use]
    pub fn resolve_call_type(&self, callee_name: &str) -> InferredType {
        // Check same-module functions first.
        if let Some(ret_type) = self.function_returns.get(callee_name) {
            return ret_type.clone();
        }

        // Check constructors.
        if let Some(class_type) = self.class_names.get(callee_name) {
            return class_type.clone();
        }

        // Check imported symbols.
        if let Some(imported) = self.imported_types.get(callee_name) {
            // If it's a Callable, return its return type.
            if let InferredType::Callable(info) = imported {
                return *info.return_type.clone();
            }
            return imported.clone();
        }

        // Check builtins.
        if let Some(builtin) = builtin_return_type(callee_name) {
            return builtin;
        }

        InferredType::Unknown
    }

    /// Resolve the return type of a method call.
    #[must_use]
    pub fn resolve_method_call_type(&self, receiver_type: &str, method_name: &str) -> InferredType {
        // Check same-module methods.
        if let Some(ret_type) = self.method_returns.get(&(receiver_type, method_name)) {
            return ret_type.clone();
        }

        // Check builtin method table.
        if let Some(builtin) = builtin_method_return_type(receiver_type, method_name) {
            return builtin;
        }

        InferredType::Unknown
    }

    /// Resolve the type of an attribute access.
    #[must_use]
    pub fn resolve_attribute_type(&self, class_name: &str, attr_name: &str) -> InferredType {
        if let Some(attr_type) = self.attr_types.get(&(class_name, attr_name)) {
            return attr_type.clone();
        }
        InferredType::Unknown
    }

    /// Infer the return type of a binary operation.
    #[must_use]
    pub fn infer_binop_type(left: &InferredType, right: &InferredType, op: &str) -> InferredType {
        match op {
            "+" | "-" | "*" | "//" | "%" | "**" => match (left, right) {
                (InferredType::Int, InferredType::Int) => {
                    if op == "/" {
                        InferredType::Float
                    } else {
                        InferredType::Int
                    }
                }
                (InferredType::Float, _) | (_, InferredType::Float) => InferredType::Float,
                (InferredType::Str, InferredType::Str) if op == "+" => InferredType::Str,
                (InferredType::List(_), InferredType::List(_)) if op == "+" => left.clone(),
                _ => InferredType::Unknown,
            },
            "/" => InferredType::Float, // True division always returns float
            "&" | "|" | "^" | "<<" | ">>" => InferredType::Int,
            "==" | "!=" | "<" | "<=" | ">" | ">=" | "in" | "not in" | "is" | "is not" => {
                InferredType::Bool
            }
            "and" | "or" => {
                // Returns one of the operands, type is their union.
                InferredType::union(left.clone(), right.clone())
            }
            _ => InferredType::Unknown,
        }
    }

    /// Infer the return type of a unary operation.
    #[must_use]
    pub fn infer_unaryop_type(operand: &InferredType, op: &str) -> InferredType {
        match op {
            "not" => InferredType::Bool,
            "-" | "+" | "~" => operand.clone(),
            _ => InferredType::Unknown,
        }
    }

    /// Infer the type of a conditional expression `a if cond else b`.
    #[must_use]
    pub fn infer_conditional_type(
        true_type: &InferredType,
        false_type: &InferredType,
    ) -> InferredType {
        if true_type == false_type {
            true_type.clone()
        } else {
            InferredType::union(true_type.clone(), false_type.clone())
        }
    }
}

/// Infer the type of a Callable from function parameters and return type.
#[must_use]
pub fn callable_type_from_function(func: &FunctionInfo, source: &str) -> InferredType {
    let param_types: Vec<InferredType> = func
        .parameters
        .iter()
        .filter_map(|param| {
            let ann_span = param.annotation_span?;
            let ann_text = slice_span(source, ann_span)?;
            Some(InferredType::from_annotation(ann_text.trim()))
        })
        .collect();

    let return_type = func
        .return_annotation_span
        .and_then(|sp| slice_span(source, sp))
        .map_or(InferredType::Unknown, |text| {
            InferredType::from_annotation(text.trim())
        });

    InferredType::Callable(CallableInfo {
        param_types,
        return_type: Box::new(return_type),
    })
}
