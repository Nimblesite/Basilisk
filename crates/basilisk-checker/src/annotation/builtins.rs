//! Implements [TYPEINF-ANNOTATION-RESOLUTION] step 4 — builtin leaves. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION
//!
//! The last step of the cascade before a name is declared unresolved. ONLY
//! names the language itself provides without an import live here; everything
//! else resolves through the module's own tables or becomes the gradual
//! `Unknown` ([TYPEINF-EXCEEDS-NOUNKNOWN]). A leaf that Python code must
//! import to use MUST NOT be added: naming one here is the symbol-naming
//! cheat, whatever the surrounding machinery.

use crate::types::InferredType;

/// Resolve a bare leaf name, EXACTLY as spelled.
///
/// The spelling is never case-folded before it reaches this table. Folding it
/// used to make typing's deprecated capitalised aliases (`List`, `Dict`,
/// `Set`, `FrozenSet`, `Tuple`, `Type`) collide with the builtin spellings
/// below and so resolve as those builtins — recognising an import-requiring
/// symbol from the characters at its use site, which is the symbol-naming
/// cheat wearing a `to_ascii_lowercase` disguise. It is deleted; do not
/// reintroduce a normalisation step here.
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
        // A bare `tuple` constrains nothing about its elements.
        "tuple" => Some(InferredType::Any),
        // Bare `type` means `type[Any]`: SOME class object. Which class is
        // gradual, but class-object-ness is not — a value positively known to
        // be an instance (`None`, `3`, `"x"`) can never be one, and the
        // resolved leaf keeps that judgment while the class-object guard in
        // the oracle keeps `x: type = C` silent ([NARROWPLAN-INTEGRATION]
        // Step 3).
        "type" => Some(InferredType::ClassObject),
        // `object` is the TOP type, not the gradual one. It accepts every value
        // exactly as `Any` does (see `is_assignable_to`), but it is a real named
        // leaf: collapsing it into `Any` made `list[object]` and `list[Any]`
        // indistinguishable, and an invariant judgment must tell them apart —
        // narrowing `list[object]` to `list[int]` is an error the spec requires
        // ([TYPEINF-NARROWING-TYPEIS]), while `list[Any]` is consistent with
        // anything.
        "object" => Some(InferredType::Object),
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

/// Does this dotted path name one of the typing modules? Used only to decide
/// whether an attribute's head is a module binding worth stripping, never to
/// decide what a MEMBER of that module means.
pub(super) fn is_typing_module(module: &str) -> bool {
    matches!(module, "typing" | "typing_extensions")
}
