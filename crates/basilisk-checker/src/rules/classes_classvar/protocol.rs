//! Implements [`classes_classvar`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! Protocol `ClassVar` conformance checks for `classes_classvar`.
//!
//! When a variable is typed as a `Protocol` with `ClassVar` attributes, the RHS
//! implementation class must have those attributes defined at the **class level**
//! (not merely as `self.x = ...` in `__init__`).
//!
//! e.g. `a: ProtoA = ProtoAImpl()` where `ProtoA` requires `y: ClassVar[str]`
//! but `ProtoAImpl` only sets `self.y = ""` in `__init__` (instance variable).
//!
//! Every verdict here is structural ([LINESCANPLAN-AST-MIGRATION]): protocol
//! bases and `ClassVar` annotations resolve through the module's import
//! cascade, and the implementation class comes from the parsed constructor
//! call rather than from the substring before the first `(`.

use basilisk_resolver::{ClassInfo, ResolvedModule};
use ruff_python_ast::Expr;

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::rules::shared::ExprIndex;

use super::helpers::{is_classvar, CODE};

/// Does this class list `Protocol` — under any spelling — among its bases?
fn is_protocol(resolver: &AnnotationResolver<'_>, cls: &ClassInfo) -> bool {
    cls.bases
        .iter()
        .any(|base| resolver.decorator_denotes(base, "Protocol"))
}

/// The class-level attributes of `cls`, each paired with whether it is
/// declared `ClassVar`.
fn class_attrs<'m>(
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
    cls: &'m ClassInfo,
) -> Vec<(&'m str, bool)> {
    cls.attributes
        .iter()
        .map(|attr| {
            let is_cv = attr
                .annotation_span
                .and_then(|span| index.expr(span))
                .is_some_and(|ann| is_classvar(resolver, ann));
            (attr.name.as_str(), is_cv)
        })
        .collect()
}

/// Check module-level annotated assignments for protocol `ClassVar` conformance.
///
/// When a variable is typed as a `Protocol` with `ClassVar` attributes, the RHS
/// implementation class must have those attributes defined at the **class level**
/// (not merely as `self.x = ...` in `__init__`).
pub(super) fn check_protocol_classvar_conformance(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Step 1: protocol classes -> the attribute names they require to be class
    // variables.
    let protocol_classvar_attrs: Vec<(&str, Vec<&str>)> = module
        .classes
        .iter()
        .filter(|cls| is_protocol(resolver, cls))
        .filter_map(|cls| {
            let names: Vec<&str> = class_attrs(resolver, index, cls)
                .into_iter()
                .filter_map(|(name, is_cv)| is_cv.then_some(name))
                .collect();
            (!names.is_empty()).then_some((cls.name.as_str(), names))
        })
        .collect();

    if protocol_classvar_attrs.is_empty() {
        return;
    }

    // Step 2: implementation classes -> their class-level attributes, each
    // flagged as `ClassVar` or not.
    let class_level_attrs: Vec<(&str, Vec<(&str, bool)>)> = module
        .classes
        .iter()
        .filter(|cls| !is_protocol(resolver, cls))
        .map(|cls| (cls.name.as_str(), class_attrs(resolver, index, cls)))
        .collect();

    // Step 3: `a: ProtoName = ClassName(...)` at module level.
    for var in &module.module_vars {
        let Some(Expr::Name(annotation)) = var.annotation_span.and_then(|span| index.expr(span))
        else {
            continue;
        };
        let Some((proto_name, required_cv_attrs)) = protocol_classvar_attrs
            .iter()
            .find(|(name, _)| *name == annotation.id.as_str())
        else {
            continue;
        };

        // The RHS must be a direct constructor call on a name this module
        // defines as a class.
        let Some(Expr::Call(call)) = var.rhs_span.and_then(|span| index.expr(span)) else {
            continue;
        };
        let Expr::Name(callee) = call.func.as_ref() else {
            continue;
        };
        let Some((impl_class_name, impl_attrs)) = class_level_attrs
            .iter()
            .find(|(name, _)| *name == callee.id.as_str())
        else {
            continue;
        };

        emit_protocol_violations(
            required_cv_attrs,
            impl_attrs,
            impl_class_name,
            proto_name,
            var.name_span,
            &module.path,
            diagnostics,
        );
    }
}

/// Emit diagnostics when a required `ClassVar` protocol attribute is either
/// absent from the implementation class or present but not declared `ClassVar`.
///
/// A protocol member annotated `ClassVar[...]` requires the implementer to
/// declare the same name as a class variable; an instance variable (plain
/// annotation) or a missing attribute both violate the protocol.
fn emit_protocol_violations(
    required_cv_attrs: &[&str],
    impl_attrs: &[(&str, bool)],
    impl_class_name: &str,
    proto_name: &str,
    name_span: basilisk_resolver::Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for cv_attr in required_cv_attrs {
        match impl_attrs.iter().find(|(name, _)| name == cv_attr) {
            // Present and correctly declared `ClassVar` — conforms.
            Some((_, true)) => {}
            // Present but declared as an instance variable — wrong kind.
            Some((_, false)) => diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Class `{impl_class_name}` is not compatible with protocol \
                     `{proto_name}`: attribute `{cv_attr}` is required to be a \
                     class variable (`ClassVar`) but is declared as an instance variable",
                ),
                name_span,
                path,
                Some(format!(
                    "Annotate `{cv_attr}` as `ClassVar[...]` in `{impl_class_name}`",
                )),
                Some(
                    "Protocol `ClassVar` attributes must be class variables in the \
                     implementation, not instance variables"
                        .to_owned(),
                ),
            )),
            // Absent entirely — not defined at class level.
            None => diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Class `{impl_class_name}` is not compatible with protocol \
                     `{proto_name}`: attribute `{cv_attr}` is required to be a \
                     class variable (`ClassVar`) but is not defined at class level",
                ),
                name_span,
                path,
                Some(format!(
                    "Define `{cv_attr}` as a class-level attribute in \
                     `{impl_class_name}` instead of assigning via `self.{cv_attr}` \
                     in `__init__`",
                )),
                Some(
                    "Protocol `ClassVar` attributes must be class-level variables \
                     in the implementation, not instance variables"
                        .to_owned(),
                ),
            )),
        }
    }
}
