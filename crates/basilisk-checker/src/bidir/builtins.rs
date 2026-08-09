//! Implements [TYPEINF-TARGET-BIDIRECTIONAL] expression inference. See
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-CHECKLIST.
//! The **centralized** builtin constructor/function/method signature table —
//! the single home the checklist demands instead of rule-local string tables
//! ("centralize builtin constructor/method signatures"). Rules migrate onto
//! this at the Integration stage; new code must never grow a private copy.

use crate::types::InferredType;

/// The result of calling a builtin, identified by the DEFINITION the callee
/// resolves to.
///
/// REBUILT from a 30-entry table of builtin SPELLINGS. That table took a bare
/// callee name and answered with a type:
///
/// ```ignore
/// "int" | "len" | "ord" | "hash" | "id"      => Int
/// "bool" | "isinstance" | "issubclass" | …   => Bool
/// "list" | "sorted"                          => List(Unknown)
/// ```
///
/// Not one of those names is reserved. Every one is an ordinary module-scope
/// binding Python lets you shadow, rebind, or import under another name, and
/// the table consulted none of that: `def len(xs) -> str: ...` still answered
/// `int`, and `from builtins import len as size; size([1])` answered nothing.
/// CLAUDE.md names this case exactly — "Builtins are not an exception ...
/// builtin uses resolve through the binding table like everything else".
///
/// Here the callee EXPRESSION is resolved through the module's bindings to a
/// [`TypingForm`], which is Basilisk's own name for a definition site and is
/// never compared against text in the file being checked. Shadowing, aliasing,
/// and qualified spellings (`builtins.int(...)`) are all handled by that one
/// resolution.
///
/// `None` means "not a builtin this table models" — including every callee the
/// bindings cannot resolve — and the caller stays `Unknown`. That is an
/// abstention about a specific callee, not the blanket `None` the deletion
/// banner forbade.
///
/// Only the constructor forms are answered. The deleted table also claimed
/// returns for `len`, `ord`, `id`, `hash`, `sorted`, and `range`, none of
/// which the canonical registry defines a form for yet; inventing one here
/// would put the spelling back. Those calls abstain until
/// `resources/typing_symbols.toml` carries their definition sites.
#[must_use]
pub fn builtin_call_return(
    bindings: &basilisk_resolver::BindingTable,
    callee: &ruff_python_ast::Expr,
) -> Option<InferredType> {
    use basilisk_resolver::TypingForm;

    match bindings.form_of_with_builtins(callee)? {
        TypingForm::IntClass => Some(InferredType::Int),
        TypingForm::StrClass => Some(InferredType::Str),
        // `complex ⊃ float ⊃ int`: the wider numeric leaves share `Float`'s
        // position in the tower ([TYPEINF-SUBTYPING-NOMINAL]).
        TypingForm::FloatClass | TypingForm::ComplexClass => Some(InferredType::Float),
        TypingForm::BoolClass => Some(InferredType::Bool),
        TypingForm::BytesClass => Some(InferredType::Bytes),
        // The three narrowing builtins are the only `builtins` FUNCTIONS the
        // registry defines, and all three answer `bool`.
        TypingForm::IsinstanceFunction
        | TypingForm::IssubclassFunction
        | TypingForm::HasattrFunction => Some(InferredType::Bool),
        // Bare container constructors constrain nothing about their elements.
        TypingForm::ListClass => Some(InferredType::List(Box::new(InferredType::Unknown))),
        TypingForm::SetClass | TypingForm::FrozensetClass => {
            Some(InferredType::Set(Box::new(InferredType::Unknown)))
        }
        TypingForm::DictClass => Some(InferredType::Dict(
            Box::new(InferredType::Unknown),
            Box::new(InferredType::Unknown),
        )),
        TypingForm::ObjectClass => Some(InferredType::Object),
        _ => None,
    }
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
