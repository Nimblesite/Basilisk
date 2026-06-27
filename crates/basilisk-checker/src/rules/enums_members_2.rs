//! Implements [`enums_members_2`] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-coercion
//! `enums_members_2`: Non-member referenced in `Literal[EnumClass.X]` annotation.
//!
//! The `Literal[EnumClass.X]` type is only valid when `X` is an actual enum
//! member. Using it with a non-member (a method, property, lambda, nested
//! class, private attribute, or `nonmember()`-wrapped attribute) is a type error.
//!
//! ```python
//! from enum import Enum, nonmember
//! from typing import Literal
//!
//! class Pet4(Enum):
//!     CAT = 1
//!     converter = lambda x: str(x)  # Non-member (lambda)
//!
//!     def speak(self) -> None: ...  # Non-member (method)
//!
//! converter: Literal[Pet4.converter]  # E — converter is not an enum member
//! speak: Literal[Pet4.speak]          # E — speak is not an enum member
//! ```

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::{guards::is_enum_class, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "enums_members_2",
    docs_url: "https://www.basilisk-python.dev/errors/enums_members_2",
};

/// Emits `enums_members_2` when a non-member is referenced in `Literal[EnumClass.X]`.
pub(crate) struct EnumNonMemberInLiteral;

impl Rule for EnumNonMemberInLiteral {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Build a map of enum class names → ClassInfo for lookup.
        let enum_classes: HashMap<&str, &ClassInfo> = module
            .classes
            .iter()
            .filter(|c| is_enum_class(c))
            .map(|c| (c.name.as_str(), c))
            .collect();

        if enum_classes.is_empty() {
            return;
        }

        let source = &module.source;
        let path = &module.path;

        // Check module-level annotated variables for `Literal[ClassName.X]` annotations.
        for var in &module.module_vars {
            let Some(ann_span) = var.annotation_span else {
                continue;
            };
            let Some(ann_text) = slice_span(source, ann_span) else {
                continue;
            };
            // Parse `Literal[ClassName.member]` from the annotation text.
            if let Some((class_name, member_name)) = parse_literal_class_member(ann_text.trim()) {
                let Some(cls) = enum_classes.get(class_name) else {
                    continue;
                };
                if is_non_member(cls, member_name) {
                    diagnostics.push(make_diagnostic(ann_span, class_name, member_name, path));
                }
            }
        }

        // Check assert_type second arguments for invalid Literal[EnumClass.X] types.
        for call in &module.assert_type_calls {
            if call.arg_count != 2 {
                continue;
            }
            let Some(expected) = &call.expected_type else {
                continue;
            };
            if let Some((class_name, member_name)) = parse_literal_class_member(expected.trim()) {
                let Some(cls) = enum_classes.get(class_name) else {
                    continue;
                };
                if is_non_member(cls, member_name) {
                    diagnostics.push(make_diagnostic(call.span, class_name, member_name, path));
                }
            }
        }
    }
}

/// Parse a `Literal[ClassName.member_name]` annotation.
///
/// Returns `Some((class_name, member_name))` when the annotation exactly
/// matches the `Literal[X.Y]` form with a single class-member reference.
///
/// Returns `None` for multi-member Literals, non-Literal annotations, or
/// annotations without a `.` in the Literal argument.
fn parse_literal_class_member(ann: &str) -> Option<(&str, &str)> {
    // Strip `Literal[` prefix and `]` suffix.
    let inner = ann.strip_prefix("Literal[")?;
    let inner = inner.strip_suffix(']')?;
    // Only handle single-item Literals (no comma means no union).
    if inner.contains(',') {
        return None;
    }
    // Must have exactly one `.` separator.
    let dot_pos = inner.find('.')?;
    let class_name = &inner[..dot_pos];
    let member_name = &inner[dot_pos + 1..];
    // Both parts must be non-empty simple identifiers.
    if class_name.is_empty() || member_name.is_empty() {
        return None;
    }
    // Class name must not contain dots or brackets.
    if class_name.contains('.') || class_name.contains('[') {
        return None;
    }
    // Member name must not contain dots or brackets.
    if member_name.contains('.') || member_name.contains('[') {
        return None;
    }
    Some((class_name, member_name))
}

/// Returns `true` when `member_name` is NOT a real enum member of `cls`.
///
/// A name is considered a non-member when:
/// - It is a method defined with `def` (unless decorated with `@member`).
/// - It starts with `__` but does not end with `__` (private name-mangled attribute).
/// - It is declared with `nonmember(...)` as the RHS.
/// - It is assigned a lambda expression.
/// - It is assigned via `staticmethod(...)` or `classmethod(...)`.
/// - It is `_value_` or `value` — these are special attributes that
///   cannot be accessed directly on enum members.
fn is_non_member(cls: &ClassInfo, member_name: &str) -> bool {
    // Private names (name-mangling): `__X` where X does not end with `__`.
    if member_name.starts_with("__") && !member_name.ends_with("__") {
        return true;
    }

    // Special enum member attributes that cannot be accessed directly.
    if member_name == "_value_" || member_name == "value" {
        return true;
    }

    // Method names defined with `def` in the class body — unless decorated with `@member`.
    if cls.method_names.iter().any(|m| m.as_str() == member_name) {
        let has_member_decorator = cls.method_decorators.iter().any(|(name, decorators)| {
            name.as_str() == member_name && decorators.iter().any(|d| d == "member")
        });
        if !has_member_decorator {
            return true;
        }
    }

    // Class body attributes explicitly declared with `nonmember(...)`, lambda, or descriptor.
    if cls.attributes.iter().any(|a| {
        a.name == member_name
            && (a.rhs_is_nonmember_call || a.rhs_is_lambda || a.rhs_is_descriptor_call)
    }) {
        return true;
    }

    false
}

fn make_diagnostic(span: Span, class_name: &str, member_name: &str, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "`{class_name}.{member_name}` is not an enum member and cannot be used in \
             `Literal[{class_name}.{member_name}]`"
        ),
        span,
        path,
        Some(format!(
            "`{member_name}` is a non-member attribute of `{class_name}` — only actual enum \
             members can appear inside `Literal[...]`"
        )),
        Some(
            "PEP 435 / typing spec: Methods, properties, descriptors, nested classes, \
             private attributes, and `nonmember()`-wrapped attributes are not enum members \
             and cannot be used in `Literal[EnumClass.X]` type expressions"
                .to_owned(),
        ),
    )
}
