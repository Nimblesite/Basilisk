//! Implements [TYPEINF-TARGET-BIDIRECTIONAL] expression inference. See
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-CHECKLIST.
//! The **centralized** builtin constructor/function/method signature table —
//! the single home the checklist demands instead of rule-local string tables
//! ("centralize builtin constructor/method signatures"). Rules migrate onto
//! this at the Integration stage; new code must never grow a private copy.

use crate::types::InferredType;

// ##########################################################################
// # DELETED BODY — `builtin_call_return`, a 30-entry table of BUILTIN      #
// # SPELLINGS. DO NOT RESTORE IT AND DO NOT RETURN `None` IN ITS PLACE.    #
// #                                                                        #
// # It took a BARE CALLEE NAME and answered with a type:                   #
// #                                                                        #
// #   "int" | "len" | "ord" | "hash" | "id"      => Int                    #
// #   "bool" | "isinstance" | "issubclass" | …   => Bool                   #
// #   "list" | "sorted"                          => List(Unknown)          #
// #   "range"                                    => Named("range")         #
// #                                                                        #
// # Not one of those names is reserved. Every one of them is an ordinary   #
// # module-scope binding that Python lets you shadow, rebind, or import    #
// # under another name, and the table consulted none of that:              #
// #                                                                        #
// #   def len(xs) -> str: ...                                              #
// #   n = len([1])            # the USER's function — table says `int`     #
// #                                                                        #
// #   from builtins import len as size                                     #
// #   n = size([1])           # `builtins.len` — table says nothing        #
// #                                                                        #
// # CLAUDE.md names this case exactly: "Builtins are not an exception —    #
// # Python lets any name be shadowed, rebound, or aliased, so builtin uses #
// # resolve through the binding table like everything else."               #
// #                                                                        #
// # The header above called this file "the single home the checklist       #
// # demands instead of rule-local string tables". Centralising a string    #
// # table does not stop it being a string table.                           #
// #                                                                        #
// # `"range" => Named("range")` is the same defect twice over, and is the  #
// # source the two DELETED `name == "range" => Int` arms in `narrow/flow`  #
// # and `bidir/engine` were reading.                                       #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
/// DELETED — panics. The signature survives only so its callers stay visible
/// as the rebuild map; see the banner above.
#[must_use]
pub fn builtin_call_return(_name: &str) -> Option<InferredType> {
    panic!(
        "basilisk-checker: `builtin_call_return` was DELETED because it identified a \
         builtin from the CHARACTERS OF THE CALLEE'S NAME, so a module that defines \
         its own `len` still got `int` and `from builtins import len as size` got \
         nothing. It panics because the real implementation — resolving the callee \
         through the binding table to its `builtins` definition, which \
         `form_of_with_builtins` already does for the shadowing question — DOES NOT \
         EXIST YET. Do not restore the name table and do not return `None` in its \
         place: `None` makes every builtin call `Unknown` while the module still \
         advertises a builtin signature table."
    )
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
