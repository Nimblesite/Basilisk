//! Implements [TYPEINF-ANNOTATION-RESOLUTION] step 4 — builtin and typeshed
//! leaves. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION
//!
//! The last step of the cascade before a name is declared unresolved. Only
//! names whose meaning is fixed by the language or the typing spec live here;
//! everything else resolves through the module's own tables or becomes the
//! gradual `Unknown` ([TYPEINF-EXCEEDS-NOUNKNOWN]).

use crate::types::{CallableInfo, InferredType};

/// Resolve a bare (already lower-cased, `typing.`-stripped) leaf name.
///
/// `None` means "not a builtin" — the caller continues the cascade.
pub(super) fn leaf(name: &str) -> Option<InferredType> {
    match name {
        "int" => Some(InferredType::Int),
        "str" => Some(InferredType::Str),
        // `complex ⊃ float ⊃ int`: the wider numeric leaves share `Float`'s
        // position in the tower ([TYPEINF-SUBTYPING-NOMINAL]).
        "float" | "complex" => Some(InferredType::Float),
        "bool" => Some(InferredType::Bool),
        "bytes" => Some(InferredType::Bytes),
        "none" => Some(InferredType::None_),
        // [TYPEINF-SPECIAL-ANY] — `Any`, `object`, and the bare gradual forms
        // are the escape hatch for assignment purposes.
        "any" | "object" | "final" | "tuple" | "type" => Some(InferredType::Any),
        // [TYPEINF-SPECIAL-NEVER] — the bottom type; `NoReturn` is its spelling
        // in return position.
        "never" | "noreturn" => Some(InferredType::Never),
        // [TYPEINF-SPECIAL-LITERALSTRING].
        "literalstring" => Some(InferredType::LiteralString),
        // A bare `Callable` is `Callable[..., Any]` (PEP 484): empty
        // `param_types` is the arbitrary-parameter form.
        "callable" => Some(InferredType::Callable(CallableInfo {
            param_types: Vec::new(),
            return_type: Box::new(InferredType::Any),
        })),
        "generator" => Some(InferredType::Generator(
            Box::new(InferredType::Any),
            Box::new(InferredType::None_),
            Box::new(InferredType::None_),
        )),
        // Bare generics are implicitly parameterised with `Any`.
        "list" => Some(InferredType::List(Box::new(InferredType::Any))),
        "dict" => Some(InferredType::Dict(
            Box::new(InferredType::Any),
            Box::new(InferredType::Any),
        )),
        "set" | "frozenset" => Some(InferredType::Set(Box::new(InferredType::Any))),
        _ => None,
    }
}

/// Does this leaf name denote a builtin type? Used to decide whether an
/// implicit assignment (`X = int`) is an alias definition or a value binding.
pub(super) fn is_builtin_type_name(name: &str) -> bool {
    leaf(name).is_some()
}

/// Modules whose members are typing special forms, so `t.Sequence` and
/// `Sequence` resolve identically once `t` is known to bind one of them.
pub(super) fn is_typing_module(module: &str) -> bool {
    matches!(module, "typing" | "typing_extensions")
}
