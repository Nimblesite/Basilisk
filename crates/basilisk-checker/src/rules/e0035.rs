//! BSK-E0035: `Required` / `NotRequired` used in an invalid context.
//!
//! PEP 655 and the typing spec restrict `Required[T]` and `NotRequired[T]` to:
//!
//! - Annotations of `TypedDict` fields
//!
//! Using them outside of a `TypedDict` body (in regular classes, function
//! parameters, variable annotations, etc.) is an error.
//!
//! Additionally, nesting `Required` or `NotRequired` inside each other is
//! forbidden even within a `TypedDict`.
//!
//! ```python
//! class NotTypedDict:
//!     x: Required[int]       # E0035 — not a TypedDict
//!
//! def func(x: NotRequired[int]) -> None:   # E0035 — not a TypedDict field
//!     ...
//!
//! class TD(TypedDict):
//!     a: Required[Required[int]]  # E0035 — nested Required
//! ```

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0035",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0035",
};

/// Returns `true` when the annotation string uses `Required[` or `NotRequired[`.
fn has_required_wrapper(ann: &str) -> bool {
    ann.contains("Required[")
}

/// Returns `true` when the annotation string uses nested Required/NotRequired.
fn has_nested_required(ann: &str) -> bool {
    ann.contains("Required[Required[")
        || ann.contains("Required[NotRequired[")
        || ann.contains("NotRequired[Required[")
        || ann.contains("NotRequired[NotRequired[")
}

fn annotation_text(source: &str, span: Option<Span>) -> Option<&str> {
    slice_span(source, span?)
}

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some(
            "`Required` and `NotRequired` may only be used in `TypedDict` field annotations"
                .to_owned(),
        ),
        note: Some(
            "PEP 655: `Required` and `NotRequired` are special forms for `TypedDict` keys only"
                .to_owned(),
        ),
    }
}

/// Returns `true` when this class or any same-module ancestor is a `TypedDict`.
///
/// We need transitive lookup because `class Child(TypedDictBase):` is also a
/// `TypedDict` even though `is_typed_dict` is only set on direct `TypedDict` inheritors.
fn is_in_typed_dict_hierarchy(cls: &ClassInfo, class_map: &HashMap<&str, &ClassInfo>) -> bool {
    if cls.is_typed_dict {
        return true;
    }
    cls.bases.iter().any(|base| {
        class_map
            .get(base.as_str())
            .is_some_and(|b| is_in_typed_dict_hierarchy(b, class_map))
    })
}

/// Emits BSK-E0035 for `Required`/`NotRequired` used outside `TypedDict` or nested.
pub(crate) struct RequiredNotRequiredContext;

impl Rule for RequiredNotRequiredContext {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Build class map for transitive TypedDict detection.
        let class_map: HashMap<&str, &ClassInfo> = module
            .classes
            .iter()
            .map(|cls| (cls.name.as_str(), cls))
            .collect();

        // Check class attributes.
        for cls in &module.classes {
            let in_typed_dict = is_in_typed_dict_hierarchy(cls, &class_map);
            for attr in &cls.attributes {
                let Some(ann) = annotation_text(source, attr.annotation_span) else {
                    continue;
                };

                if in_typed_dict {
                    // Inside a TypedDict: only nested usage is an error.
                    if has_nested_required(ann) {
                        diagnostics.push(make_diagnostic(
                            format!(
                                "`Required` and `NotRequired` cannot be nested in `{}`",
                                attr.name
                            ),
                            attr.name_span,
                            path,
                        ));
                    }
                } else {
                    // Outside a TypedDict: any usage is an error.
                    if has_required_wrapper(ann) {
                        diagnostics.push(make_diagnostic(
                            format!(
                                "`Required`/`NotRequired` is not allowed on attribute `{}` \
                                 outside a `TypedDict`",
                                attr.name
                            ),
                            attr.name_span,
                            path,
                        ));
                    }
                }
            }
        }

        // Check function parameters.
        for func in &module.functions {
            for param in &func.parameters {
                let Some(ann) = annotation_text(source, param.annotation_span) else {
                    continue;
                };
                if has_required_wrapper(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`Required`/`NotRequired` is not allowed on parameter `{}`",
                            param.name
                        ),
                        param.name_span,
                        path,
                    ));
                }
            }
            // Also check vararg and kwarg.
            for param in func.vararg.iter().chain(func.kwarg.iter()) {
                let Some(ann) = annotation_text(source, param.annotation_span) else {
                    continue;
                };
                if has_required_wrapper(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`Required`/`NotRequired` is not allowed on parameter `{}`",
                            param.name
                        ),
                        param.name_span,
                        path,
                    ));
                }
            }
        }
    }
}
