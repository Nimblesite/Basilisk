//! BSK-E0083: `TypeVarTuple` must be unpacked with `*` operator.
//!
//! When a `TypeVarTuple` is used in a generic class base list or as a direct
//! type annotation, it must be unpacked using the `*` operator.  Using a
//! `TypeVarTuple` without unpacking is invalid per PEP 646.
//!
//! ```python
//! from typing import Generic, TypeVarTuple
//!
//! Ts = TypeVarTuple("Ts")
//!
//! # BAD
//! class Cls(Generic[Ts]):  # E: TypeVarTuple must be unpacked with *
//!     ...
//!
//! def f(*args: Ts) -> None:  # E: TypeVarTuple must be unpacked with *
//!     ...
//!
//! # GOOD
//! class Cls2(Generic[*Ts]):  # OK
//!     ...
//!
//! def f2(*args: *Ts) -> None:  # OK
//!     ...
//! ```

use std::collections::HashSet;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0083",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0083",
};

fn make_diag(msg: String, span: basilisk_resolver::Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: msg,
        span,
        path: path.to_owned(),
        help: Some(
            "Unpack the `TypeVarTuple` with `*`, e.g. `Generic[*Ts]` or `*args: *Ts`".to_owned(),
        ),
        note: Some(
            "PEP 646: TypeVarTuple must always be used with the `*` unpack operator".to_owned(),
        ),
        provenance: None,
    }
}

fn span_text(source: &str, span: Option<basilisk_resolver::Span>) -> Option<&str> {
    let span = span?;
    slice_span(source, span)
}

/// Check whether an annotation text contains `tuple[Ts]` where `Ts` is a
/// `TypeVarTuple` used without the `*` unpack operator.
///
/// Returns a diagnostic if the violation is found.
fn check_tuple_subscript_unpack(
    ann: &str,
    tvt_names: &HashSet<&str>,
    span: basilisk_resolver::Span,
    path: &str,
) -> Option<Diagnostic> {
    // Look for `tuple[` (case-sensitive — Python's builtin is lowercase).
    let tuple_prefix = "tuple[";
    let start = ann.find(tuple_prefix)?;
    let inner_start = start + tuple_prefix.len();

    // Find the matching closing bracket.
    let inner = &ann[inner_start..];
    let close = find_matching_bracket(inner)?;
    let subscript_content = &inner[..close];

    // Check each comma-separated element for a bare TypeVarTuple name.
    for element in subscript_content.split(',') {
        let element = element.trim();
        if tvt_names.contains(element) {
            return Some(make_diag(
                format!("`TypeVarTuple` `{element}` must be unpacked with `*` inside `tuple[...]`"),
                span,
                path,
            ));
        }
    }
    None
}

/// Find the index of the matching `]` for content starting after `[`.
fn find_matching_bracket(content: &str) -> Option<usize> {
    let mut depth = 0u32;
    for (idx, ch) in content.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                if depth == 0 {
                    return Some(idx);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

/// Emits BSK-E0083 when a `TypeVarTuple` is used without unpacking.
pub(crate) struct TypeVarTupleUnpackRequired;

impl Rule for TypeVarTupleUnpackRequired {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Collect all TypeVarTuple names defined in this module.
        let tvt_names: HashSet<&str> = module
            .typevar_calls
            .iter()
            .filter(|tv| tv.is_typevartuple)
            .map(|tv| tv.name.as_str())
            .collect();

        if tvt_names.is_empty() {
            return;
        }

        let path = &module.path;
        let source = &module.source;

        // Check class generic parameters: if a class uses a TypeVarTuple name in its
        // generic parameter list without the `*` unpack, it's an error.
        for cls in &module.classes {
            for param in &cls.generic_params {
                if !param.is_typevartuple && tvt_names.contains(param.name.as_str()) {
                    diagnostics.push(make_diag(
                        format!(
                            "`TypeVarTuple` `{}` must be unpacked with `*` in generic parameter list",
                            param.name
                        ),
                        param.span,
                        path,
                    ));
                }
            }
        }

        // Check function parameters and varargs: if a parameter's annotation is exactly
        // a bare TypeVarTuple name (not preceded by `*`), it's an error.
        // Also check for `tuple[Ts]` — TypeVarTuple inside tuple[] without unpack.
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
                let ann_trimmed = ann.trim();
                // Annotation is exactly a bare TypeVarTuple name (no leading `*`).
                if !ann_trimmed.starts_with('*') && tvt_names.contains(ann_trimmed) {
                    diagnostics.push(make_diag(
                        format!(
                            "`TypeVarTuple` `{ann_trimmed}` must be unpacked with `*` in annotation for `{}`",
                            param.name
                        ),
                        param.name_span,
                        path,
                    ));
                }
                // Check for `tuple[Ts]` — TypeVarTuple inside tuple subscript without `*`.
                if let Some(diag) =
                    check_tuple_subscript_unpack(ann_trimmed, &tvt_names, param.name_span, path)
                {
                    diagnostics.push(diag);
                }
            }

            // Check return annotation for bare TypeVarTuple or tuple[Ts] without unpack.
            if let Some(ret_ann) = span_text(source, func.return_annotation_span) {
                let ret_trimmed = ret_ann.trim();
                if !ret_trimmed.starts_with('*') && tvt_names.contains(ret_trimmed) {
                    let span = func.return_annotation_span.unwrap_or(func.name_span);
                    diagnostics.push(make_diag(
                        format!(
                            "`TypeVarTuple` `{ret_trimmed}` must be unpacked with `*` in return annotation",
                        ),
                        span,
                        path,
                    ));
                }
                if let Some(span) = func.return_annotation_span {
                    if let Some(diag) =
                        check_tuple_subscript_unpack(ret_trimmed, &tvt_names, span, path)
                    {
                        diagnostics.push(diag);
                    }
                }
            }
        }
    }
}
