//! Shared guard predicates used across multiple rules.
//!
//! These predicates identify Python typing patterns where strict annotation
//! enforcement should be suspended because the construct has well-defined PEP
//! semantics that legitimately omit annotations.

use basilisk_resolver::{ClassInfo, FunctionInfo};

/// Returns `true` when a function is in a "stub context" — a context where
/// annotation enforcement (E0001, E0002, E0004) should be skipped.
///
/// A stub context is any of:
/// - A non-`@overload` function whose body is a pure stub (only `...`, `pass`,
///   or a docstring): Protocol method stubs, abstract placeholders, `.pyi`-style
///   inline stubs.  **`@overload` variants are excluded** — they must carry
///   annotations because their signatures drive overload resolution.
/// - A function decorated with `@abstractmethod` (even with a non-stub body).
/// - A method inside a `Protocol` class (interface contract, not implementation).
pub(crate) fn is_stub_context(func: &FunctionInfo, classes: &[ClassInfo]) -> bool {
    // @overload variants MUST be annotated — their signatures drive type resolution.
    if func.decorators.iter().any(|d| d == "overload") {
        return false;
    }
    // Pure stub bodies (only `...` / `pass`) are exempt — covers Protocol stubs
    // and abstract placeholders that legitimately omit annotations.
    if func.is_stub_body {
        return true;
    }
    // Non-stub abstractmethod bodies are also exempt.
    if func.decorators.iter().any(|d| d == "abstractmethod") {
        return true;
    }
    // Protocol methods are interface contracts, not implementations.
    func.class_name.as_ref().is_some_and(|cls_name| {
        classes
            .iter()
            .find(|c| &c.name == cls_name)
            .is_some_and(is_protocol_class)
    })
}

/// Returns `true` when a class is an Enum subclass.
///
/// Enum members are unannotated by design — their type is `Literal[EnumClass.member]`,
/// synthesised by the Enum metaclass.  Firing E0005 on them is a false positive.
pub(crate) fn is_enum_class(class: &ClassInfo) -> bool {
    class.bases.iter().any(|b| {
        matches!(
            b.as_str(),
            "Enum" | "IntEnum" | "StrEnum" | "Flag" | "IntFlag"
        )
    })
}

/// Returns `true` when a class is a `Protocol` subclass.
///
/// Protocol attributes are interface specifications, not concrete class variables.
/// Unannotated names in a Protocol body are structural members, not bugs.
pub(crate) fn is_protocol_class(class: &ClassInfo) -> bool {
    class.bases.iter().any(|b| b == "Protocol")
}
