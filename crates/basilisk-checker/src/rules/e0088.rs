//! BSK-E0088: Invalid use of `TypedDict` as a runtime type.
//!
//! `TypedDict` classes cannot be used with `isinstance()` or as a `TypeVar` bound.
//!
//! PEP 589 specifies that:
//! - `TypedDict` type objects cannot be used in `isinstance()` tests.
//! - `TypedDict` (the abstract base) cannot be used as a bound for a `TypeVar`.
//!
//! ```python
//! from typing import TypeVar, TypedDict
//!
//! class Movie(TypedDict):
//!     name: str
//!     year: int
//!
//! movie: Movie = {"name": "Blade Runner", "year": 1982}
//!
//! if isinstance(movie, Movie):  # E: TypedDict in isinstance
//!     pass
//!
//! T = TypeVar("T", bound=TypedDict)  # E: TypedDict as TypeVar bound
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0088",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0088",
};

/// Emits BSK-E0088 for invalid runtime uses of `TypedDict`.
pub(crate) struct TypedDictRuntimeViolation;

impl Rule for TypedDictRuntimeViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // isinstance(x, TypedDictClass) violations
        for &span in &module.isinstance_typeddict_violations {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: "TypedDict classes cannot be used as the second argument to `isinstance()`; \
                          TypedDict types are not runtime classes"
                    .to_owned(),
                span,
                path: module.path.clone(),
                help: Some(
                    "Use a regular `dict` check or restructure to avoid runtime TypedDict inspection"
                        .to_owned(),
                ),
                note: Some(
                    "PEP 589: TypedDict type objects have no runtime type identity".to_owned(),
                ),
            });
        }

        // TypeVar("T", bound=TypedDict) violations
        let source = &module.source;
        for tv in &module.typevar_calls {
            if !tv.has_bound {
                continue;
            }
            let Some(bound_text) = extract_bound_text(source, tv.span) else {
                continue;
            };
            if bound_text == "TypedDict" || bound_text == "typing.TypedDict" {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "`TypedDict` cannot be used as a `TypeVar` bound for `{}`; \
                         TypedDict is a special form, not a class",
                        tv.name
                    ),
                    span: tv.span,
                    path: module.path.clone(),
                    help: Some(
                        "Use a concrete TypedDict subclass or a Protocol as the bound instead"
                            .to_owned(),
                    ),
                    note: Some(
                        "PEP 589: TypedDict is a special typing form and cannot be used as a TypeVar bound"
                            .to_owned(),
                    ),
                });
            }
        }
    }
}

/// Extract the bound text from a `TypeVar("Name", bound=X)` call in source.
fn extract_bound_text(source: &str, span: basilisk_resolver::Span) -> Option<String> {
    let call_text = source.get(span.start as usize..span.end as usize)?;
    let bound_idx = call_text.find("bound=")?;
    let after_bound = &call_text[bound_idx + "bound=".len()..];

    let mut depth = 0u32;
    let mut end = after_bound.len();
    for (idx, ch) in after_bound.char_indices() {
        match ch {
            '[' | '(' => depth = depth.saturating_add(1),
            ']' | ')' => {
                if depth == 0 {
                    end = idx;
                    break;
                }
                depth = depth.saturating_sub(1);
            }
            ',' if depth == 0 => {
                end = idx;
                break;
            }
            _ => {}
        }
    }
    let bound_text = after_bound[..end].trim();
    let bound_text = bound_text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(bound_text);
    if bound_text.is_empty() {
        return None;
    }
    Some(bound_text.to_owned())
}
