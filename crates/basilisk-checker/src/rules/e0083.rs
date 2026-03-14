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
    }
}

fn span_text(source: &str, span: Option<basilisk_resolver::Span>) -> Option<&str> {
    let span = span?;
    slice_span(source, span)
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
            }
        }
    }
}
