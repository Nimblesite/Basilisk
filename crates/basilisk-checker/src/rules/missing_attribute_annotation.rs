//! Implements [BSK-0005] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-MISSING
//! BSK-0005: Missing class attribute type annotation.
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

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-0005",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-0005",
};

/// Emits BSK-0005 for every unannotated class attribute.
pub(crate) struct MissingAttributeAnnotation;

impl Rule for MissingAttributeAnnotation {
    fn opt_in_spec(&self) -> Option<crate::rule_tags::OptInSpec> {
        Some(crate::rule_tags::OptInSpec {
            code: CODE.code,
            tags: &["strictness"],
        })
    }

    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Collect all TypeVar names (module-level and class-body) so we can
        // exempt unannotated TypeVar assignments like `T = TypeVar("T")` from BSK-0005.
        let typevar_names: std::collections::HashSet<&str> = module
            .typevar_calls
            .iter()
            .map(|tv| tv.name.as_str())
            .collect();

        // Likewise exempt `X = TypeAliasType("X", ...)` alias definitions: these
        // are type-system declarations (static type `typing.TypeAliasType`), not
        // data attributes, so requiring an annotation is wrong. The resolver
        // already collects these call-sites recursively (including class bodies).
        let alias_names: std::collections::HashSet<&str> = module
            .type_alias_type_calls
            .iter()
            .map(|c| c.lhs_name.as_str())
            .collect();

        module
            .classes
            .iter()
            .filter(|class| {
                !class.is_enum && !class.is_protocol && !class.is_namedtuple
            })
            .for_each(|class| {
                check_class(
                    class,
                    &module.path,
                    &typevar_names,
                    &alias_names,
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
    alias_names: &std::collections::HashSet<&str>,
    all_classes: &[ClassInfo],
    out: &mut Vec<Diagnostic>,
) {
    class
        .attributes
        .iter()
        .filter(|attr| {
            !attr.has_annotation
                && !typevar_names.contains(attr.name.as_str())
                && !alias_names.contains(attr.name.as_str())
                && !is_inferrable_literal(&attr.rhs_kind)
                && !class
                    .pep695_type_param_names
                    .iter()
                    .any(|p| p == &attr.name)
                && !parent_has_annotated_attr(&attr.name, class, all_classes)
        })
        .for_each(|attr| out.push(make_diagnostic(attr, &class.name, path)));
}

/// Returns `true` when the RHS is a literal whose type is fully inferrable
/// without an annotation: a scalar (int, float, str, bool, bytes, None) or a
/// tuple literal whose elements are all themselves inferrable (so `()` and
/// `("a", "b")` — e.g. dataclass `__match_args__` — are exempt). Empty
/// list/dict/set are deliberately excluded: their element types are unknown
/// without an annotation, so they still require one.
fn is_inferrable_literal(rhs: &RhsKind) -> bool {
    match rhs {
        RhsKind::IntLiteral
        | RhsKind::FloatLiteral
        | RhsKind::StrLiteral
        | RhsKind::BoolLiteral
        | RhsKind::BytesLiteral
        | RhsKind::NoneValue => true,
        RhsKind::Tuple(elems) => elems.iter().all(is_inferrable_literal),
        _ => false,
    }
}

/// Returns `true` when any ancestor class declares an attribute with the same
/// name *and* that declaration carries a type annotation.  This allows
/// subclasses to override inherited attributes without re-annotating.
fn parent_has_annotated_attr(
    attr_name: &str,
    class: &ClassInfo,
    all_classes: &[ClassInfo],
) -> bool {
    let resolve = |base: &str| all_classes.iter().find(|candidate| candidate.name == base);
    let declares_annotated = |candidate: &ClassInfo| {
        candidate
            .attributes
            .iter()
            .any(|a| a.name == attr_name && a.has_annotation)
    };
    // Only ancestors count — the class's own (unannotated) declaration is the
    // one under scrutiny, so the walk starts from each base.
    class.bases.iter().any(|base_name| {
        resolve(base_name).is_some_and(|candidate| {
            super::shared::class_or_base_matches(candidate, &resolve, &declares_annotated)
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
