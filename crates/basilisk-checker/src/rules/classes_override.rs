//! Implements [`classes_override`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `classes_override`: Incompatible method override.
//!
//! When a class method marked with `@override` has a different parameter
//! signature or return type than the corresponding method in a same-module
//! base class, Basilisk reports an incompatible override.
//!
//! The check compares annotation text extracted from the source for non-self
//! parameters and the return type.  The `self`/`cls` parameter is always
//! skipped since its type naturally differs between base and child class.
//!
//! ```python
//! class Base:
//!     def process(self: Base, data: str) -> str: ...
//!
//! class Child(Base):
//!     @override
//!     def process(self: Child, data: int) -> int: ...  # E0016
//! ```

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, FunctionInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "classes_override",
    docs_url: "https://www.basilisk-python.dev/errors/classes_override",
};

/// Emits `classes_override` for `@override` methods with incompatible signatures.
pub(crate) struct IncompatibleOverride;

impl Rule for IncompatibleOverride {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Build map: class_name → method_name → &FunctionInfo
        let method_map: HashMap<(&str, &str), &FunctionInfo> = module
            .functions
            .iter()
            .filter_map(|func| {
                func.class_name
                    .as_deref()
                    .map(|cls| ((cls, func.name.as_str()), func))
            })
            .collect();

        // Build set of class names for same-module lookup.
        let class_names: Vec<&str> = basilisk_resolver::collect_names(&module.classes);

        module.classes.iter().for_each(|child| {
            check_class(
                child,
                &method_map,
                &class_names,
                &module.source,
                &module.path,
                diagnostics,
            );
        });
    }
}

fn check_class(
    child: &ClassInfo,
    method_map: &HashMap<(&str, &str), &FunctionInfo>,
    class_names: &[&str],
    source: &str,
    path: &str,
    out: &mut Vec<Diagnostic>,
) {
    for base_name in &child.bases {
        if !class_names.contains(&base_name.as_str()) {
            continue;
        }

        for (method_name, decorators) in &child.method_decorators {
            // Only check methods with @override.
            let has_override = decorators
                .iter()
                .any(|d| d == "override" || d.ends_with(".override"));
            if !has_override {
                continue;
            }

            let Some(child_func) = method_map.get(&(child.name.as_str(), method_name.as_str()))
            else {
                continue;
            };
            let Some(base_func) = method_map.get(&(base_name.as_str(), method_name.as_str()))
            else {
                continue;
            };

            if signatures_incompatible(child_func, base_func, source) {
                out.push(make_diagnostic(child_func, method_name, &child.name, path));
            }
        }
    }
}

/// Returns `true` when the non-self parameters or return type differ.
fn signatures_incompatible(child: &FunctionInfo, base: &FunctionInfo, source: &str) -> bool {
    // Skip the first parameter (self/cls) since its type naturally differs.
    let child_params = skip_self_param(&child.parameters);
    let base_params = skip_self_param(&base.parameters);

    if child_params.len() != base_params.len() {
        return true;
    }

    // Compare non-self parameter annotation texts.
    let params_differ = child_params.iter().zip(base_params.iter()).any(|(cp, bp)| {
        annotations_conflict(
            annotation_text(source, cp.annotation_span),
            annotation_text(source, bp.annotation_span),
        )
    });

    if params_differ {
        return true;
    }

    // Compare return annotation texts.
    annotations_conflict(
        annotation_text(source, child.return_annotation_span),
        annotation_text(source, base.return_annotation_span),
    )
}

/// Two annotation positions conflict only when BOTH are present and differ.
///
/// An unannotated side is implicitly `Any` — consistent with anything, so its
/// absence can never prove incompatibility ([TYPEINF-TARGET-GRADUAL]:
/// removing an annotation must never introduce a new error).
fn annotations_conflict(child: Option<&str>, base: Option<&str>) -> bool {
    match (child, base) {
        (None, _) | (_, None) => false,
        (Some(child_text), Some(base_text)) => child_text != base_text,
    }
}

/// Returns a slice of parameters with the leading `self`/`cls` removed (if present).
fn skip_self_param(
    params: &[basilisk_resolver::ParameterInfo],
) -> &[basilisk_resolver::ParameterInfo] {
    match params.first() {
        Some(p) if p.name == "self" || p.name == "cls" => params.get(1..).unwrap_or_default(),
        _ => params,
    }
}

/// Extract annotation text from source given an optional span.
fn annotation_text(source: &str, span: Option<basilisk_resolver::Span>) -> Option<&str> {
    slice_span(source, span?)
}

fn make_diagnostic(
    func: &FunctionInfo,
    method_name: &str,
    class_name: &str,
    path: &str,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Method `{method_name}` in `{class_name}` has an incompatible signature with the \
             base-class method it overrides"
        ),
        func.name_span,
        path,
        Some(format!(
            "Update `{method_name}` to have the same parameter types and return type as the \
             base-class definition, or remove the `@override` decorator"
        )),
        Some(
            "An `@override` method must be type-compatible with its base-class counterpart"
                .to_owned(),
        ),
    )
}
