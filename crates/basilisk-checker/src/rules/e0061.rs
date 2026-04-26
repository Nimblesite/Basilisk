//! BSK-E0061: `assert_type` with `Literal[EnumClass.MEMBER]` on an enum-typed parameter.
//!
//! When a function parameter is typed as a plain enum class (e.g. `f: CustomFlags`)
//! and `assert_type(f, Literal[CustomFlags.MEMBER])` is used, the assertion is always
//! wrong: the static type of `f` is `CustomFlags`, not a `Literal` member of it.
//!
//! This is especially important for `Flag` enum classes, where literal expansion is
//! explicitly prohibited by the typing spec — flag values can be combined arbitrarily
//! and therefore no individual member can be the narrowed type.
//!
//! ```python
//! from enum import Flag
//! from typing import Literal, assert_type
//!
//! class CustomFlags(Flag):
//!     FLAG1 = 1
//!
//! def test(f: CustomFlags) -> None:
//!     assert_type(f, Literal[CustomFlags.FLAG1])  # E
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0061",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0061",
};

const PRIMITIVES: &[&str] = &[
    "int", "str", "float", "bool", "bytes", "complex", "None", "Any", "object",
    "list", "dict", "set", "tuple", "type", "bytearray", "memoryview", "Never",
    "NoReturn",
];

/// Returns `true` when `t` is a bare simple identifier (no union, no subscript, no primitive).
///
/// These are class names that are not known built-in types — e.g. `CustomFlags`, `Color`.
fn is_bare_class_name(t: &str) -> bool {
    let t = t.trim();
    // Must not be empty
    if t.is_empty() {
        return false;
    }
    // Must not contain union or subscript operators
    if t.contains('|') || t.contains('[') || t.contains('.') || t.contains(' ') {
        return false;
    }
    // Must start with an alphabetic character or underscore
    let mut chars = t.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    if !chars.all(|c| c.is_alphanumeric() || c == '_') {
        return false;
    }
    !PRIMITIVES.contains(&t)
}

/// Returns `true` when `expected` is `Literal[class_name.MEMBER]` for the given `class_name`.
///
/// This checks that the expected type is a `Literal` containing a single member
/// of the same class that the actual type refers to.
fn is_literal_of_class_member(expected: &str, class_name: &str) -> bool {
    let prefix = format!("Literal[{class_name}.");
    if !expected.starts_with(&prefix) {
        return false;
    }
    // There must be at least one character after "ClassName." and the string must end with "]"
    let after_prefix = &expected[prefix.len()..];
    after_prefix.ends_with(']') && after_prefix.len() > 1
}

/// Emits BSK-E0061 for `assert_type(f, Literal[EnumClass.MEMBER])` where `f: EnumClass`.
pub(crate) struct AssertTypeEnumLiteralMismatch;

impl Rule for AssertTypeEnumLiteralMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let path = &module.path;

        for call in &module.assert_type_calls {
            // Only check calls with exactly 2 arguments.
            if call.arg_count != 2 {
                continue;
            }

            let (Some(actual), Some(expected)) = (&call.actual_type, &call.expected_type) else {
                continue;
            };

            // actual must be a bare class name (no union, no subscript)
            if !is_bare_class_name(actual) {
                continue;
            }

            // expected must be Literal[actual.MEMBER]
            if !is_literal_of_class_member(expected, actual) {
                continue;
            }

            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "`assert_type` mismatch: actual type is `{actual}` \
                     but expected `{expected}` — enum types cannot be narrowed \
                     to a `Literal` member in this context"
                ),
                span: call.span,
                path: path.clone(),
                help: Some(format!(
                    "The type of the expression is `{actual}`, not `{expected}`. \
                     Enum classes (especially `Flag` subclasses) do not support \
                     literal narrowing to individual members."
                )),
                note: Some(
                    "PEP 675 / typing spec: `Flag` enum classes do not expand to \
                     `Literal` member types — use the enum class type directly."
                        .to_owned(),
                ),
            });
        }
    }
}
