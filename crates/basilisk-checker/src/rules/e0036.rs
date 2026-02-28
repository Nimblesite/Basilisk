//! BSK-E0036: `ClassVar` used in an invalid context.
//!
//! PEP 526 and the typing spec restrict `ClassVar[T]` to:
//!
//! - Annotations of class body attributes (class variables)
//!
//! Using `ClassVar` outside a class body (in function parameters, return types,
//! local variable annotations, or module-level variable annotations) is an error.
//! Additionally, nesting `ClassVar` inside another type constructor (e.g.
//! `Final[ClassVar[int]]` or `list[ClassVar[int]]`) is forbidden.
//!
//! Note: `Annotated[ClassVar[T], ...]` is a valid exception.
//!
//! ```python
//! class MyClass:
//!     bad9: Final[ClassVar[int]] = 3     # E0036 — ClassVar cannot be nested
//!     bad10: list[ClassVar[int]] = []    # E0036 — ClassVar cannot be nested
//!
//!     def method1(self, a: ClassVar[int]):   # E0036 — ClassVar not allowed here
//!         ...
//!
//!     def method2(self) -> ClassVar[int]:    # E0036 — ClassVar not allowed here
//!         ...
//!
//! bad11: ClassVar[int] = 3              # E0036 — ClassVar not allowed at module level
//! bad12: TypeAlias = ClassVar[str]      # E0036 — ClassVar not allowed here
//! ```

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0036",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0036",
};

/// Returns the text slice for a span within the source.
fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    let span = span?;
    source.get(span.start as usize..span.end as usize)
}

/// Returns `true` when the annotation text contains `ClassVar[` at all —
/// used for contexts where ANY `ClassVar` usage is invalid (function params,
/// return types, module-level annotations).
fn has_classvar(ann: &str) -> bool {
    ann.contains("ClassVar[") || ann.contains("ClassVar ")
}

/// Returns `true` when the annotation text contains `ClassVar` nested inside
/// another type constructor.  `Annotated[ClassVar[...], ...]` is excluded
/// because that is a valid usage per the typing spec.
///
/// Pattern: `[ClassVar[` appears in the annotation (meaning something wraps it)
/// AND the annotation does not begin with `Annotated[`.
fn has_nested_classvar(ann: &str) -> bool {
    ann.contains("[ClassVar[") && !ann.starts_with("Annotated[")
}

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some("`ClassVar` is only valid as a class body attribute annotation".to_owned()),
        note: Some(
            "PEP 526: `ClassVar` cannot appear in function signatures, local variables, \
             or module-level annotations, and cannot be nested inside another type"
                .to_owned(),
        ),
    }
}

/// Emits BSK-E0036 for `ClassVar` used in an invalid context.
pub(crate) struct ClassVarInvalidContext;

impl Rule for ClassVarInvalidContext {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // --- Class attributes: detect nested ClassVar ---
        // e.g. `Final[ClassVar[int]]`, `list[ClassVar[int]]`
        // (Valid top-level usage like `ClassVar[int]` is not flagged here.)
        for cls in &module.classes {
            for attr in &cls.attributes {
                let Some(ann) = span_text(source, attr.annotation_span) else {
                    continue;
                };
                if has_nested_classvar(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` cannot be nested inside another type in attribute `{}`",
                            attr.name
                        ),
                        attr.name_span,
                        path,
                    ));
                }
            }
        }

        // --- Function parameters: ClassVar not allowed ---
        for func in &module.functions {
            for param in func
                .parameters
                .iter()
                .chain(func.vararg.iter())
                .chain(func.kwarg.iter())
            {
                let Some(ann) = span_text(source, param.annotation_span) else {
                    continue;
                };
                if has_classvar(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` is not allowed in parameter annotation for `{}`",
                            param.name
                        ),
                        param.name_span,
                        path,
                    ));
                }
            }

            // --- Function return type: ClassVar not allowed ---
            let Some(ret_ann) = span_text(source, func.return_annotation_span) else {
                continue;
            };
            if has_classvar(ret_ann) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`ClassVar` is not allowed in the return annotation of `{}`",
                        func.name
                    ),
                    func.name_span,
                    path,
                ));
            }
        }

        // --- Module-level variables: ClassVar not allowed ---
        for var in &module.module_vars {
            // Check annotation span (for `bad11: ClassVar[int] = 3`)
            if let Some(ann) = span_text(source, var.annotation_span) {
                if has_classvar(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` is not allowed in module-level annotation for `{}`",
                            var.name
                        ),
                        var.name_span,
                        path,
                    ));
                    // Don't double-report for the same variable
                    continue;
                }
            }
            // Check RHS span (for `bad12: TypeAlias = ClassVar[str]`)
            if let Some(rhs) = span_text(source, var.rhs_span) {
                if has_classvar(rhs) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` is not allowed in right-hand side of module-level \
                             assignment for `{}`",
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
