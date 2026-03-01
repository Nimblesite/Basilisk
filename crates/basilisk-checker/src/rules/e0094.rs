//! BSK-E0094: `Self` type used in an invalid location.
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

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0094",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0094",
};

fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    let span = span?;
    source.get(span.start as usize..span.end as usize)
}

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some(
            "`Self` is only valid inside class method annotations and class variable annotations"
                .to_owned(),
        ),
        note: Some(
            "PEP 673: `Self` binds to the class in which it is defined; \
             it cannot be used at module scope, in staticmethods, or in metaclass methods"
                .to_owned(),
        ),
    }
}

/// Returns `true` when `text` contains the word `Self` as a standalone identifier.
fn contains_self(text: &str) -> bool {
    let bytes = text.as_bytes();
    let target = b"Self";
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if bytes[i..i + 4] == *target {
            let before_ok = i == 0
                || (!bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_');
            let after_ok = i + 4 >= bytes.len()
                || (!bytes[i + 4].is_ascii_alphanumeric() && bytes[i + 4] != b'_');
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

/// Emits BSK-E0094 when `Self` is used in a location where it has no valid binding.
pub(crate) struct SelfInvalidLocation;

impl Rule for SelfInvalidLocation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Collect TypeVar names (needed for check: self annotated with TypeVar).
        let typevar_names: HashSet<&str> = module
            .typevar_calls
            .iter()
            .filter(|tv| !tv.is_typevartuple && !tv.is_paramspec)
            .map(|tv| tv.name.as_str())
            .collect();

        // Build set of class names that are metaclasses (inherit from `type`).
        let metaclass_names: HashSet<&str> = module
            .classes
            .iter()
            .filter(|cls| cls.bases.iter().any(|b| b == "type"))
            .map(|cls| cls.name.as_str())
            .collect();

        // Check functions for Self in invalid locations.
        for func in &module.functions {
            match &func.class_name {
                None => {
                    // Module-level function (or nested function inside a method, which also has
                    // class_name = None due to the resolver's design). In both cases `Self` lacks
                    // a defined binding.
                    check_func_annotations_for_self(
                        func,
                        source,
                        path,
                        "module-level function",
                        diagnostics,
                    );
                }
                Some(class_name) => {
                    let is_static = func.decorators.iter().any(|d| d == "staticmethod");
                    let is_in_metaclass = metaclass_names.contains(class_name.as_str());

                    if is_static {
                        // @staticmethod: no `self` is bound, so `Self` is meaningless.
                        check_func_annotations_for_self(
                            func,
                            source,
                            path,
                            "static method",
                            diagnostics,
                        );
                    } else if is_in_metaclass {
                        // Metaclass method: `Self` would refer to a metaclass instance (a class
                        // object), which is almost never intended and is disallowed by PEP 673.
                        check_func_annotations_for_self(
                            func,
                            source,
                            path,
                            "metaclass method",
                            diagnostics,
                        );
                    } else {
                        // Regular class method — `Self` is valid in most positions, EXCEPT when
                        // the `self`/`cls` parameter is explicitly annotated with a TypeVar.
                        // In that case the binding of `Self` is ambiguous.
                        if let Some(first) = func.parameters.first() {
                            if (first.name == "self" || first.name == "cls")
                                && first.annotation_span.is_some()
                            {
                                if let Some(ann) = span_text(source, first.annotation_span) {
                                    let ann_t = ann.trim();
                                    // `self: TypeVar` (but not `self: Self`) → invalid binding.
                                    if typevar_names.contains(ann_t) && ann_t != "Self" {
                                        if let Some(ret_ann) =
                                            span_text(source, func.return_annotation_span)
                                        {
                                            if contains_self(ret_ann) {
                                                diagnostics.push(make_diagnostic(
                                                    format!(
                                                        "Return type `Self` is invalid in `{}`: \
                                                         the `self` parameter is annotated with \
                                                         TypeVar `{ann_t}`, so `Self` has no \
                                                         defined class binding",
                                                        func.name
                                                    ),
                                                    func.name_span,
                                                    path,
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check module-level variables for Self in annotations or TypeAlias RHS.
        for var in &module.module_vars {
            let Some(ann) = span_text(source, var.annotation_span) else {
                continue;
            };
            let ann_trimmed = ann.trim();

            if contains_self(ann_trimmed) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`Self` is not valid outside a class body; \
                         annotation of `{}` uses `Self`",
                        var.name
                    ),
                    var.name_span,
                    path,
                ));
                continue;
            }

            // TypeAlias with Self in RHS: `TupleSelf: TypeAlias = tuple[Self]`
            if ann_trimmed == "TypeAlias" {
                if let Some(rhs) = span_text(source, var.rhs_span) {
                    if contains_self(rhs) {
                        diagnostics.push(make_diagnostic(
                            format!(
                                "`Self` cannot appear in a `TypeAlias` definition \
                                 at module scope; `{}` uses `Self`",
                                var.name
                            ),
                            var.name_span,
                            path,
                        ));
                    }
                }
            }
        }

        // Check class base class expressions for Self.
        // Both `class Foo(Self)` and `class Foo(Bar[Self])` are invalid.
        for cls in &module.classes {
            if cls.base_expression_names.iter().any(|n| n == "Self") {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`Self` cannot be used as a base class or in base class type \
                         arguments for `{}`",
                        cls.name
                    ),
                    cls.name_span,
                    path,
                ));
            }
        }
    }
}
