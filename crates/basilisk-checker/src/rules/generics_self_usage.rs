//! Implements [`generics_self_usage`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_self_usage`: `Self` type used in an invalid location.
//!
//! PEP 673 defines `Self` as a special type that refers to the current class.
//! It is only valid in specific locations:
//!
//! - Method parameter annotations (including `self` and `cls`)
//! - Method return type annotations
//! - Class variable annotations inside the class body
//! - Nested within other types at those locations
//!
//! Invalid locations (detected here):
//!
//! - Return types or parameter annotations of module-level functions
//! - Module-level variable annotations (`bar: Self`)
//! - `TypeAlias` definitions whose RHS contains `Self`
//! - Base class expressions (`class Foo(Bar[Self])` or `class Foo(Self)`)
//! - `@staticmethod` method annotations (no `self` to bind to)
//! - Method annotations in metaclasses (classes inheriting from `type`)
//! - Return type annotation when `self` is explicitly annotated with a `TypeVar`
//!   (e.g. `def f(self: TFoo2) -> Self:` — binding is ambiguous)
//!
//! ```python
//! # E — not within a class
//! def foo(bar: Self) -> Self: ...
//! bar: Self
//!
//! class Base:
//!     @staticmethod
//!     def make() -> Self: ...  # E — staticmethod has no Self binding
//!
//! class MyMeta(type):
//!     def __new__(cls, *args: Any) -> Self: ...  # E — metaclass
//! ```

use std::collections::HashSet;

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_self_usage",
    docs_url: "https://www.basilisk-python.dev/errors/generics_self_usage",
};

fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    let span = span?;
    slice_span(source, span)
}

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        message,
        span,
        path,
        Some(
            "`Self` is only valid inside class method annotations and class variable annotations"
                .to_owned(),
        ),
        Some(
            "PEP 673: `Self` binds to the class in which it is defined; \
             it cannot be used at module scope, in staticmethods, or in metaclass methods"
                .to_owned(),
        ),
    )
}

/// Returns `true` when `text` contains the word `Self` as a standalone identifier.
fn contains_self(text: &str) -> bool {
    let bytes = text.as_bytes();
    let target = b"Self";
    let mut i = 0;
    while i + 4 <= bytes.len() {
        // The tail `i += 1` already covers a miss, so the window check stays a
        // plain `if`; advancing `i` inside a `let … else` scrutinee would read
        // and write it within one expression.
        if bytes.get(i..i + 4) == Some(&target[..]) {
            let before_ok = i == 0
                || bytes
                    .get(i - 1)
                    .is_none_or(|&b| !b.is_ascii_alphanumeric() && b != b'_');
            let after_ok = i + 4 >= bytes.len()
                || bytes
                    .get(i + 4)
                    .is_none_or(|&b| !b.is_ascii_alphanumeric() && b != b'_');
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Check all annotations in `func` for invalid `Self` usage and push diagnostics.
fn check_func_annotations_for_self(
    func: &FunctionInfo,
    source: &str,
    path: &str,
    context: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for param in func
        .parameters
        .iter()
        .chain(func.vararg.iter())
        .chain(func.kwarg.iter())
    {
        if let Some(ann) = span_text(source, param.annotation_span) {
            if contains_self(ann) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`Self` is not valid in a {context}; \
                         parameter `{}` of `{}` uses `Self`",
                        param.name, func.name
                    ),
                    param.name_span,
                    path,
                ));
            }
        }
    }
    if let Some(ret_ann) = span_text(source, func.return_annotation_span) {
        if contains_self(ret_ann) {
            diagnostics.push(make_diagnostic(
                format!(
                    "`Self` is not valid in a {context}; \
                     return type of `{}` uses `Self`",
                    func.name
                ),
                func.name_span,
                path,
            ));
        }
    }
}

/// Emits `generics_self_usage` when `Self` is used in a location where it has no valid binding.
pub(crate) struct SelfInvalidLocation;

impl Rule for SelfInvalidLocation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        check_self_invalid_locations(module, diagnostics);
    }
}

fn check_typevar_annotated_self(
    func: &FunctionInfo,
    source: &str,
    path: &str,
    typevar_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(first) = func.parameters.first() else {
        return;
    };
    if (first.name != "self" && first.name != "cls") || first.annotation_span.is_none() {
        return;
    }
    let Some(ann) = span_text(source, first.annotation_span) else {
        return;
    };
    let ann_t = ann.trim();
    if !typevar_names.contains(ann_t) || ann_t == "Self" {
        return;
    }
    if let Some(ret_ann) = span_text(source, func.return_annotation_span) {
        if contains_self(ret_ann) {
            diagnostics.push(make_diagnostic(
                format!(
                    "Return type `Self` is invalid in `{}`: \
                     the `self` parameter is annotated with \
                     TypeVar `{ann_t}`, so `Self` has no defined class binding",
                    func.name
                ),
                func.name_span,
                path,
            ));
        }
    }
}

fn check_functions_self_usage(
    module: &ResolvedModule,
    source: &str,
    path: &str,
    metaclass_names: &HashSet<&str>,
    typevar_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for func in &module.functions {
        match &func.class_name {
            None => {
                // A closure lexically nested inside a class method still has a
                // valid `Self` binding (PEP 673), so only genuinely module-level
                // functions are flagged here.  [generics_self_usage]
                if !func.nested_in_class {
                    check_func_annotations_for_self(
                        func,
                        source,
                        path,
                        "module-level function",
                        diagnostics,
                    );
                }
            }
            Some(class_name) => {
                let is_static = super::shared::decorator_spelled(&func.decorators, "staticmethod");
                if is_static {
                    check_func_annotations_for_self(
                        func,
                        source,
                        path,
                        "static method",
                        diagnostics,
                    );
                } else if metaclass_names.contains(class_name.as_str()) {
                    check_func_annotations_for_self(
                        func,
                        source,
                        path,
                        "metaclass method",
                        diagnostics,
                    );
                } else {
                    check_typevar_annotated_self(func, source, path, typevar_names, diagnostics);
                }
            }
        }
    }
}

fn check_module_vars_self_usage(
    module: &ResolvedModule,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in &module.module_vars {
        let Some(ann) = span_text(source, var.annotation_span) else {
            continue;
        };
        let ann_trimmed = ann.trim();
        if contains_self(ann_trimmed) {
            diagnostics.push(make_diagnostic(
                format!(
                    "`Self` is not valid outside a class body; annotation of `{}` uses `Self`",
                    var.name
                ),
                var.name_span,
                path,
            ));
            continue;
        }
        if ann_trimmed == "TypeAlias" {
            if let Some(rhs) = span_text(source, var.rhs_span) {
                if contains_self(rhs) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`Self` cannot appear in a `TypeAlias` definition at module scope; \
                             `{}` uses `Self`",
                            var.name
                        ),
                        var.name_span,
                        path,
                    ));
                }
            }
        }
    }
}

fn check_class_bases_self_usage(
    module: &ResolvedModule,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for cls in &module.classes {
        if cls.base_expression_names.iter().any(|n| n == "Self") {
            diagnostics.push(make_diagnostic(
                format!(
                    "`Self` cannot be used as a base class or in base class type arguments for `{}`",
                    cls.name
                ),
                cls.name_span,
                path,
            ));
        }
    }
}

fn check_self_invalid_locations(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let source = &module.source;
    let path = &module.path;

    let typevar_names: HashSet<&str> = module
        .typevar_calls
        .iter()
        .filter(|tv| !tv.is_typevartuple && !tv.is_paramspec)
        .map(|tv| tv.name.as_str())
        .collect();

    let metaclass_names: HashSet<&str> = module
        .classes
        .iter()
        .filter(|cls| cls.bases.iter().any(|b| b == "type"))
        .map(|cls| cls.name.as_str())
        .collect();

    check_functions_self_usage(
        module,
        source,
        path,
        &metaclass_names,
        &typevar_names,
        diagnostics,
    );
    check_module_vars_self_usage(module, source, path, diagnostics);
    check_class_bases_self_usage(module, path, diagnostics);
}
