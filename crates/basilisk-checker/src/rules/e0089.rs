//! BSK-E0089: Invalid key or value type in `TypedDict` assignment.
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

use basilisk_resolver::{ResolvedModule, TypedDictKeyViolationKind};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0089",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0089",
};

/// Emits BSK-E0089 for invalid key or value-type violations on `TypedDict` instances.
pub(crate) struct TypedDictKeyValidation;

impl Rule for TypedDictKeyValidation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
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
            };

            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message,
                span: violation.span,
                path: module.path.clone(),
                help: None,
                note: None,
            });
        }
    }
}
