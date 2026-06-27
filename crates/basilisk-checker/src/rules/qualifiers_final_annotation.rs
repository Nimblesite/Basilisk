//! Implements [BSK-E0044] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-immutability
//! BSK-E0044: `Final` used in an invalid position.
//!
//! PEP 591 restricts `Final[T]` to:
//!
//! - Module-level variable annotations (`x: Final[int] = 1`)
//! - Class body attribute annotations (`VALUE: Final[int] = 1`)
//! - Instance attribute annotations in `__init__` (`self.x: Final[int] = 1`)
//!
//! The following are all errors:
//!
//! 1. `Final` used in a function parameter annotation
//! 2. `Final` nested inside another type constructor (e.g. `list[Final[int]]`)
//! 3. `Final[ClassVar[...]]` or `ClassVar[Final[...]]` — mutually exclusive
//! 4. `Final[T1, T2]` — more than one type argument
//! 5. Bare `Final` (no type arg, no initializer) at module level
//!
//! ```python
//! x: list[Final[int]] = []    # E — Final nested in list
//! def f(x: Final[int]): ...   # E — Final in param
//! VALUE2: ClassVar[Final] = 1 # E — Final with ClassVar
//! BAD1: Final                  # E — bare Final, no assignment
//! BAD2: Final[str, int] = ""  # E — too many type args
//! ```

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0044",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0044",
};

fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    slice_span(source, span?)
}

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic(
        CODE.clone(),
        message,
        span,
        path,
        Some(
            "`Final` is only valid as the outermost qualifier in variable or attribute annotations",
        ),
        Some("PEP 591: `Final` cannot be nested, used in parameters, or combined with `ClassVar`"),
    )
}

/// Returns `true` when an annotation text contains `Final` nested inside another
/// type constructor — e.g. `list[Final[int]]`, `Optional[Final[int]]`.
///
/// `Final[...]` at the top-level (starts with `Final`) is NOT nested.
/// `ClassVar[Final[...]]` is handled separately (and exempt in dataclasses).
/// `Annotated[Final[...], ...]` is explicitly valid per PEP 591.
fn has_nested_final(ann: &str) -> bool {
    // Annotated[Final[...], ...] is explicitly valid — skip it.
    if ann.starts_with("Annotated[") {
        return false;
    }
    // ClassVar[Final[...]] is handled by has_classvar_wrapping_final — skip here
    // to avoid double-reporting (and to respect the dataclass exemption).
    if ann.starts_with("ClassVar[") {
        return false;
    }
    // Has `[Final[` somewhere — meaning Final is not the outermost wrapper.
    ann.contains("[Final[") || ann.contains("[Final ")
}

/// Returns `true` when the annotation is `ClassVar[Final...]` — Final inside `ClassVar`.
fn has_classvar_wrapping_final(ann: &str) -> bool {
    ann.starts_with("ClassVar[")
        && (ann.contains("Final[") || ann.contains("Final]") || ann.contains("Final,"))
}

/// Returns `true` when the annotation is `Final[ClassVar...]` — `ClassVar` inside Final.
fn has_final_wrapping_classvar(ann: &str) -> bool {
    ann.starts_with("Final[") && ann.contains("ClassVar")
}

/// Returns `true` when the annotation is `Final[T1, T2, ...]` — multiple type args.
///
/// Detects by counting commas at the top level inside `Final[...]`.
fn has_final_multiple_type_args(ann: &str) -> bool {
    if !ann.starts_with("Final[") {
        return false;
    }
    // Extract contents of Final[...]
    let inner_start = "Final[".len();
    let Some(inner_end) = ann.rfind(']') else {
        return false;
    };
    if inner_end <= inner_start {
        return false;
    }
    let Some(inner) = ann.get(inner_start..inner_end) else {
        return false;
    };
    // Count top-level commas (depth 0 = inside Final[...] but not nested further)
    let mut depth = 0i32;
    let mut top_commas = 0u32;
    for ch in inner.chars() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ',' if depth == 0 => top_commas += 1,
            _ => {}
        }
    }
    top_commas >= 1
}

/// Emits BSK-E0044 for `Final` used in an invalid position.
pub(crate) struct FinalInvalidPosition;

impl Rule for FinalInvalidPosition {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;
        let path = &module.path;

        // --- Function parameters: Final not allowed ---
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
                if ann.starts_with("Final[") || ann == "Final" {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`Final` is not allowed in parameter annotation for `{}`",
                            param.name
                        ),
                        param.name_span,
                        path,
                    ));
                }
            }
        }

        // --- Module-level variables ---
        for var in &module.module_vars {
            let Some(ann) = span_text(source, var.annotation_span) else {
                continue;
            };
            let ann = ann.trim();

            // Bare `Final` with no assignment (rhs_span is None, no type arg)
            if ann == "Final" && var.rhs_span.is_none() {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Bare `Final` annotation for `{}` requires an explicit type argument or initializer",
                        var.name
                    ),
                    var.name_span,
                    path,
                ));
            }

            // `Final[T1, T2]` — too many type args
            if has_final_multiple_type_args(ann) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`Final` accepts at most one type argument for `{}`",
                        var.name
                    ),
                    var.name_span,
                    path,
                ));
            }

            // `Final` nested inside another type (e.g. `list[Final[int]]`)
            if has_nested_final(ann) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`Final` cannot be nested inside another type constructor for `{}`",
                        var.name
                    ),
                    var.name_span,
                    path,
                ));
            }
        }

        // --- Class attributes ---
        for cls in &module.classes {
            // PEP 681 / dataclasses spec: `ClassVar[Final[int]]` is explicitly valid
            // in dataclasses as a way to declare a final class variable.
            let is_dataclass = cls.is_dataclass;

            for attr in &cls.attributes {
                let Some(ann) = span_text(source, attr.annotation_span) else {
                    continue;
                };
                let ann = ann.trim();

                // `ClassVar[Final[...]]` — invalid except in dataclasses
                if !is_dataclass && has_classvar_wrapping_final(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`Final` cannot be used inside `ClassVar` for attribute `{}`",
                            attr.name
                        ),
                        attr.name_span,
                        path,
                    ));
                }

                // `Final[ClassVar[...]]`
                if has_final_wrapping_classvar(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`ClassVar` cannot be used inside `Final` for attribute `{}`",
                            attr.name
                        ),
                        attr.name_span,
                        path,
                    ));
                }

                // Final nested in another type
                if has_nested_final(ann) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`Final` cannot be nested inside another type constructor for `{}`",
                            attr.name
                        ),
                        attr.name_span,
                        path,
                    ));
                }
            }
        }
    }
}
