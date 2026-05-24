//! Implements [BSK-E0005] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-missing
//! BSK-E0005: Missing class attribute type annotation.
//!
//! Every class attribute declared in the class body must have an explicit type
//! annotation.  Without one, Basilisk cannot verify assignments to the
//! attribute and cannot produce accurate stub types.
//!
//! Enum subclasses and Protocol subclasses are exempt: Enum members have
//! metaclass-synthesised `Literal[...]` types, and Protocol attributes are
//! interface specifications rather than concrete class variables.

use basilisk_resolver::{AttributeInfo, ClassInfo, ResolvedModule, RhsKind};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::{
    guards::{is_enum_class, is_namedtuple_class, is_protocol_class},
    Rule,
};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0005",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0005",
};

/// Emits BSK-E0005 for every unannotated class attribute.
pub(crate) struct MissingAttributeAnnotation;

impl Rule for MissingAttributeAnnotation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Collect all TypeVar names (module-level and class-body) so we can
        // exempt unannotated TypeVar assignments like `T = TypeVar("T")` from E0005.
        let typevar_names: std::collections::HashSet<&str> = module
            .typevar_calls
            .iter()
            .map(|tv| tv.name.as_str())
            .collect();

        module
            .classes
            .iter()
            .filter(|class| {
                !is_enum_class(class) && !is_protocol_class(class) && !is_namedtuple_class(class)
            })
            .for_each(|class| {
                check_class(
                    class,
                    &module.path,
                    &typevar_names,
                    &module.classes,
                    diagnostics,
                );
            });
    }
}

fn check_class(
    class: &ClassInfo,
    path: &str,
    typevar_names: &std::collections::HashSet<&str>,
    all_classes: &[ClassInfo],
    out: &mut Vec<Diagnostic>,
) {
    class
        .attributes
        .iter()
        .filter(|attr| {
            !attr.has_annotation
                && !typevar_names.contains(attr.name.as_str())
                && !is_scalar_literal(&attr.rhs_kind)
                && !parent_has_annotated_attr(&attr.name, class, all_classes)
        })
        .for_each(|attr| out.push(make_diagnostic(attr, &class.name, path)));
}

/// Returns `true` when the RHS is a scalar literal whose type is trivially
/// inferrable (int, float, str, bool, bytes, None).
fn is_scalar_literal(rhs: &RhsKind) -> bool {
    matches!(
        rhs,
        RhsKind::IntLiteral
            | RhsKind::FloatLiteral
            | RhsKind::StrLiteral
            | RhsKind::BoolLiteral
            | RhsKind::BytesLiteral
            | RhsKind::NoneValue
    )
}

/// Returns `true` when any ancestor class declares an attribute with the same
/// name *and* that declaration carries a type annotation.  This allows
/// subclasses to override inherited attributes without re-annotating.
fn parent_has_annotated_attr(
    attr_name: &str,
    class: &ClassInfo,
    all_classes: &[ClassInfo],
) -> bool {
    class.bases.iter().any(|base_name| {
        all_classes.iter().any(|candidate| {
            candidate.name == *base_name
                && (candidate
                    .attributes
                    .iter()
                    .any(|a| a.name == attr_name && a.has_annotation)
                    || parent_has_annotated_attr(attr_name, candidate, all_classes))
        })
    })
}

fn make_diagnostic(attr: &AttributeInfo, class_name: &str, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Missing type annotation for attribute `{}` in class `{}`",
            attr.name, class_name
        ),
        attr.name_span,
        path,
        Some(format!("Add a type annotation: `{}: <type>`", attr.name)),
        Some("In Basilisk, all class attributes require explicit type annotations".to_owned()),
    )
}
