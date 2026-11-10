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
        // [TYPEINF-SPECIAL-ANY] — `Any` and the bare gradual forms are the
        // escape hatch for assignment purposes.
        "any" | "final" | "tuple" => Some(InferredType::Any),
        // Bare `type` means `type[Any]`: SOME class object. Which class is
        // gradual, but class-object-ness is not — a value positively known to
        // be an instance (`None`, `3`, `"x"`) can never be one, and the
        // nominal leaf keeps that judgment while the class-object guard in the
        // oracle keeps `x: type = C` silent ([NARROWPLAN-INTEGRATION] Step 3).
        "type" => Some(InferredType::Named("type".to_owned())),
        // `object` is the TOP type, not the gradual one. It accepts every value
        // exactly as `Any` does (see `is_assignable_to`), but it is a real named
        // leaf: collapsing it into `Any` made `list[object]` and `list[Any]`
        // indistinguishable, and an invariant judgment must tell them apart —
        // narrowing `list[object]` to `list[int]` is an error the spec requires
        // ([TYPEINF-NARROWING-TYPEIS]), while `list[Any]` is consistent with
        // anything.
        "object" => Some(InferredType::Named("object".to_owned())),
        // [TYPEINF-SPECIAL-NEVER] — the bottom type; `NoReturn` is its spelling
        // in return position.
        "never" | "noreturn" => Some(InferredType::Never),
        // [TYPEINF-SPECIAL-LITERALSTRING].
        "literalstring" => Some(InferredType::LiteralString),
        // A bare `Callable` is `Callable[..., Any]` (PEP 484): the gradual-tail
        // marker is the arbitrary-parameter form.
        "callable" => Some(InferredType::Callable(CallableInfo {
            param_types: crate::types::gradual_params(Vec::new()),
            return_type: Box::new(InferredType::Any),
        })),
        // A bare `TypeForm` is `TypeForm[Any]` (PEP 747), for the same reason a
        // bare `Callable` is `Callable[..., Any]`. Left as a plain name it
        // stopped denoting a type form at all, and the RHS of
        // `x: TypeForm = <expr>` was then never validated as a type expression.
        "typeform" => Some(InferredType::TypeForm(Box::new(InferredType::Any))),
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
