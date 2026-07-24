//! Implements [TYPEINF-TARGET-BIDIRECTIONAL] expression inference. See
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-CHECKLIST.
//! The **centralized** builtin constructor/function/method signature table —
//! the single home the checklist demands instead of rule-local string tables
//! ("centralize builtin constructor/method signatures"). Rules migrate onto
//! this at the Integration stage; new code must never grow a private copy.

use crate::types::InferredType;

/// Return type of calling a builtin constructor or function by bare name.
///
/// Only names whose return type is fixed regardless of arguments are listed —
/// argument-dependent builtins (`abs`, `max`, `next`, …) stay out rather than
/// guessing ([TYPEINF-EXCEEDS-NOUNKNOWN]).
#[must_use]
pub fn builtin_call_return(name: &str) -> Option<InferredType> {
    Some(match name {
        "int" | "len" | "ord" | "hash" | "id" => InferredType::Int,
        "float" => InferredType::Float,
        "str" | "repr" | "format" | "chr" | "hex" | "oct" | "bin" | "ascii" | "input" => {
            InferredType::Str
        }
        "bool" | "isinstance" | "issubclass" | "callable" | "hasattr" => InferredType::Bool,
        "bytes" | "bytearray" => InferredType::Bytes,
        "list" | "sorted" => InferredType::List(Box::new(InferredType::Unknown)),
        "dict" => InferredType::Dict(
            Box::new(InferredType::Unknown),
            Box::new(InferredType::Unknown),
        ),
        "set" | "frozenset" => InferredType::Set(Box::new(InferredType::Unknown)),
        "range" => InferredType::Named("range".to_owned()),
        "print" => InferredType::None_,
        _ => return None,
    })
}

/// Return type of calling a method on a receiver of a known builtin type.
///
/// Covers the argument-independent core of `str`/`list`/`dict`/`set`
/// methods; anything else answers `None` (the caller stays `Unknown`).
#[must_use]
pub fn builtin_method_return(receiver: &InferredType, method: &str) -> Option<InferredType> {
    match receiver {
        InferredType::Str | InferredType::LiteralString | InferredType::Literal(_) => {
            str_method_return(method)
        }
        InferredType::List(elem) => list_method_return(method, elem),
        InferredType::Dict(key, value) => dict_method_return(method, key, value),
        InferredType::Set(_) => set_method_return(method),
        _ => None,
    }
}

/// `str` methods with argument-independent returns.
fn str_method_return(method: &str) -> Option<InferredType> {
    Some(match method {
        "upper" | "lower" | "strip" | "lstrip" | "rstrip" | "title" | "capitalize" | "casefold"
        | "swapcase" | "replace" | "format" | "join" | "zfill" | "center" | "ljust" | "rjust"
        | "expandtabs" | "removeprefix" | "removesuffix" => InferredType::Str,
        "split" | "rsplit" | "splitlines" => InferredType::List(Box::new(InferredType::Str)),
        "startswith" | "endswith" | "isdigit" | "isalpha" | "isalnum" | "isspace" | "isupper"
        | "islower" | "istitle" | "isnumeric" | "isdecimal" | "isidentifier" | "isprintable"
        | "isascii" => InferredType::Bool,
        "find" | "rfind" | "index" | "rindex" | "count" => InferredType::Int,
        "encode" => InferredType::Bytes,
        _ => return None,
    })
}

/// `list` methods with argument-independent returns.
fn list_method_return(method: &str, elem: &InferredType) -> Option<InferredType> {
    Some(match method {
        "append" | "extend" | "insert" | "remove" | "clear" | "sort" | "reverse" => {
            InferredType::None_
        }
        "pop" => elem.clone(),
        "count" | "index" => InferredType::Int,
        "copy" => InferredType::List(Box::new(elem.clone())),
        _ => return None,
    })
}

/// `dict` methods with argument-independent returns.
fn dict_method_return(
    method: &str,
    key: &InferredType,
    value: &InferredType,
) -> Option<InferredType> {
    Some(match method {
        "keys" => InferredType::List(Box::new(key.clone())),
        "values" => InferredType::List(Box::new(value.clone())),
        "clear" | "update" => InferredType::None_,
        "copy" => InferredType::Dict(Box::new(key.clone()), Box::new(value.clone())),
        "get" => InferredType::Optional(Box::new(value.clone())),
        _ => return None,
    })
}

/// `set` methods with argument-independent returns.
fn set_method_return(method: &str) -> Option<InferredType> {
    Some(match method {
        "add" | "discard" | "remove" | "clear" | "update" => InferredType::None_,
        "isdisjoint" | "issubset" | "issuperset" => InferredType::Bool,
        _ => return None,
    })
}
