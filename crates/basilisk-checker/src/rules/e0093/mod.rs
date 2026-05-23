//! BSK-E0093: Invalid key or value type in `TypedDict` assignment.
//!
//! PEP 589 defines `TypedDict` as a typed dict with a fixed set of keys and associated types.
//! This rule detects:
//!
//! 1. Subscript assignments with invalid (non-existent) keys.
//! 2. Subscript assignments where the value type is incompatible with the declared field type.
//! 3. Annotated dict-literal assignments that contain invalid keys or are missing required keys.
//!
//! ```python
//! from typing import TypedDict
//!
//! class Movie(TypedDict):
//!     name: str
//!     year: int
//!
//! movie: Movie = {"name": "Blade Runner", "year": 1982}
//!
//! movie["director"] = "Ridley Scott"  # E: invalid key
//! movie["year"] = "1982"              # E: wrong value type
//! movie2: Movie = {"title": "Blade Runner", "year": 1982}  # E: invalid/missing keys
//! ```

mod type_consistency;

use basilisk_resolver::{ResolvedModule, TypedDictKeyViolationKind};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0093",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0093",
};

/// Emits BSK-E0093 for invalid key or value-type violations on `TypedDict` instances.
pub(crate) struct TypedDictKeyValidation;

impl Rule for TypedDictKeyValidation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        type_consistency::check_typeddict_assignability(module, diagnostics);

        for violation in &module.typeddict_key_violations {
            let message = match &violation.kind {
                TypedDictKeyViolationKind::InvalidSubscriptKey { key } => format!(
                    "`{}` is not a valid key for `TypedDict` `{}`",
                    key, violation.class_name
                ),
                TypedDictKeyViolationKind::WrongSubscriptValueType { key, expected } => format!(
                    "Value assigned to `TypedDict` `{}` field `{}` has the wrong type; \
                     expected `{expected}`",
                    violation.class_name, key
                ),
                TypedDictKeyViolationKind::InvalidDictLiteral {
                    invalid_keys,
                    missing_keys,
                } => {
                    let mut parts = Vec::new();
                    if !invalid_keys.is_empty() {
                        let ks = invalid_keys
                            .iter()
                            .map(|k| format!("`{k}`"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        parts.push(format!(
                            "invalid key{} {} for `TypedDict` `{}`",
                            if invalid_keys.len() == 1 { "" } else { "s" },
                            ks,
                            violation.class_name
                        ));
                    }
                    if !missing_keys.is_empty() {
                        let ks = missing_keys
                            .iter()
                            .map(|k| format!("`{k}`"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        parts.push(format!(
                            "missing required key{} {}",
                            if missing_keys.len() == 1 { "" } else { "s" },
                            ks
                        ));
                    }
                    parts.join("; ")
                }
                TypedDictKeyViolationKind::SubscriptReadInvalidKey { key } => format!(
                    "Key `{}` is not a valid key for `TypedDict` `{}`",
                    key, violation.class_name
                ),
                TypedDictKeyViolationKind::NonLiteralDictKey => format!(
                    "Dict literal for `TypedDict` `{}` contains a non-literal key; \
                     all keys must be string literals",
                    violation.class_name
                ),
                TypedDictKeyViolationKind::DisallowedMethodCall { method } => format!(
                    "`TypedDict` `{}` does not support `.{}()` — \
                     TypedDicts have a fixed schema",
                    violation.class_name, method
                ),
                TypedDictKeyViolationKind::DeleteSubscript => format!(
                    "Cannot delete a key from `TypedDict` `{}` — \
                     TypedDicts have a fixed schema",
                    violation.class_name
                ),
            };

            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                message,
                violation.span,
                &module.path,
                None,
                None,
            ));
        }
    }
}
