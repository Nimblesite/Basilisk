//! Implements [`enums_expansion`] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-coercion
//! `enums_expansion`: `assert_type` with `Literal[Enum.MEMBER]` on enum-typed param.
//!
//! This rule detects when `assert_type()` is used with a `Literal[Enum.MEMBER]` type
//! on a parameter that is already typed as the enum itself. This is redundant and
//! indicates a misunderstanding of enum typing semantics.
//!
//! ```python
//! from enum import Enum
//! from typing import assert_type, Literal
//!
//! class Status(Enum):
//!     ACTIVE = 1
//!     INACTIVE = 2
//!
//! def process(status: Status) -> None:
//!     assert_type(status, Literal[Status.ACTIVE])  # E0061 — redundant narrowing
//!     assert_type(status, Status)                  # OK — correct usage
//! ```

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::{guards::is_enum_class, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "enums_expansion",
    docs_url: "https://www.basilisk-python.dev/errors/enums_expansion",
};

/// Emits `enums_expansion` for `assert_type` with `Literal[Enum.MEMBER]` on enum-typed param.
pub(crate) struct AssertTypeEnumLiteralMismatch;

impl Rule for AssertTypeEnumLiteralMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let path = &module.path;

        // Build a map of enum class names → ClassInfo for lookup
        let enum_classes: std::collections::HashMap<&str, _> = module
            .classes
            .iter()
            .filter(|c| is_enum_class(c))
            .map(|c| (c.name.as_str(), c))
            .collect();

        if enum_classes.is_empty() {
            return;
        }

        // Check assert_type calls for the specific pattern
        for call in &module.assert_type_calls {
            if call.arg_count != 2 {
                continue;
            }

            // Get the expected type from the assert_type call
            let Some(expected_type) = &call.expected_type else {
                continue;
            };

            // Check if this is a Literal[Enum.MEMBER] pattern
            if let Some((class_name, member_name)) = parse_literal_enum_member(expected_type.trim())
            {
                // Verify this is a valid enum class
                let Some(enum_class) = enum_classes.get(class_name) else {
                    continue;
                };

                // Check if the member exists in the enum
                if !is_valid_enum_member(enum_class, member_name) {
                    continue; // This would be handled by E0067
                }

                // Only flag when the first argument is already typed as the full
                // enum class.  If the actual type is unknown, a union, a specific
                // member literal, or anything other than the plain enum class name,
                // the `assert_type` may be intentional (e.g. confirming narrowing
                // or testing member identity).
                let is_enum_typed_param = call
                    .actual_type
                    .as_ref()
                    .is_some_and(|actual| actual.trim() == class_name);
                if !is_enum_typed_param {
                    continue;
                }

                diagnostics.push(make_diagnostic(call.span, class_name, member_name, path));
            }
        }
    }
}

/// Parse a `Literal[EnumClass.MEMBER]` annotation.
///
/// Returns `Some((class_name, member_name))` when the annotation exactly
/// matches the `Literal[EnumClass.MEMBER]` form.
fn parse_literal_enum_member(ann: &str) -> Option<(&str, &str)> {
    // Strip `Literal[` prefix and `]` suffix
    let inner = ann.strip_prefix("Literal[")?;
    let inner = inner.strip_suffix(']')?;

    // Only handle single-item Literals (no comma means no union)
    if inner.contains(',') {
        return None;
    }

    // Must have exactly one `.` separator
    let dot_pos = inner.find('.')?;
    let class_name = &inner[..dot_pos];
    let member_name = &inner[dot_pos + 1..];

    // Both parts must be non-empty simple identifiers
    if class_name.is_empty() || member_name.is_empty() {
        return None;
    }

    // Class name must not contain dots or brackets
    if class_name.contains('.') || class_name.contains('[') {
        return None;
    }

    // Member name must not contain dots or brackets
    if member_name.contains('.') || member_name.contains('[') {
        return None;
    }

    Some((class_name, member_name))
}

/// Check if a member name is a valid enum member
fn is_valid_enum_member(class_info: &basilisk_resolver::ClassInfo, member_name: &str) -> bool {
    // Check if it's a regular enum member (not a method, property, etc.)
    // For now, we'll assume any attribute that's not a method is a valid member
    // This is a simplification - real implementation would need more sophisticated checking
    !class_info
        .method_names
        .iter()
        .any(|m| m.as_str() == member_name)
}

fn make_diagnostic(span: Span, class_name: &str, member_name: &str, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Redundant `assert_type` with `Literal[{class_name}.{member_name}]` on enum-typed parameter"
        ),
        span,
        path,
        Some(format!(
            "Use `assert_type(param, {class_name})` instead — enum parameters already have the correct type"
        )),
        Some(
            "Narrowing an enum-typed parameter to a specific member with `Literal[Enum.MEMBER]` \
             is redundant and indicates a misunderstanding of enum typing semantics"
                .to_owned(),
        ),
    )
}
