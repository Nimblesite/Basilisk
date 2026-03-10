//! BSK-E0058: `Annotated[...]` requires at least two arguments.
//!
//! PEP 593 requires `Annotated` to be subscripted with at least two arguments:
//! a type and one or more metadata values. `Annotated[int]` with only a single
//! argument is a type error.
//!
//! ```python
//! from typing import Annotated
//! bad: Annotated[int]  # E — only one argument
//! ```

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0058",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0058",
};

fn make_diag(span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: "`Annotated` requires at least two arguments: a type and metadata".to_owned(),
        span,
        path: path.to_owned(),
        help: Some("Use `Annotated[Type, metadata]` with at least one metadata value".to_owned()),
        note: Some("PEP 593: `Annotated[X]` with a single argument is invalid".to_owned()),
    }
}

/// Returns `true` when `ann` is an `Annotated[...]` subscript with only one argument
/// (no top-level comma inside the brackets).
fn is_annotated_single_arg(ann: &str) -> bool {
    // Find `Annotated[`
    let start = if let Some(pos) = ann.find("Annotated[") {
        pos + "Annotated[".len()
    } else {
        return false;
    };

    // Find the matching closing `]`
    let bytes = ann.as_bytes();
    let mut depth = 1i32;
    let mut i = start;
    let mut end = start;
    while i < bytes.len() {
        match bytes[i] {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }
    if end <= start {
        return false;
    }
    let inner = &ann[start..end];
    !has_top_level_comma(inner)
}

fn has_top_level_comma(s: &str) -> bool {
    let mut depth = 0i32;
    for b in s.bytes() {
        match b {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b',' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

fn check_annotation(ann: &str, name_span: Span, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    if is_annotated_single_arg(ann) {
        diagnostics.push(make_diag(name_span, path));
    }
}

/// Emits BSK-E0058 when `Annotated[X]` has fewer than two arguments.
pub(crate) struct AnnotatedTooFewArguments;

impl Rule for AnnotatedTooFewArguments {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Module-level variable annotations
        for var in &module.module_vars {
            let Some(ann_span) = var.annotation_span else {
                continue;
            };
            let Some(ann) = source.get(ann_span.start as usize..ann_span.end as usize) else {
                continue;
            };
            check_annotation(ann.trim(), var.name_span, path, diagnostics);
        }

        // Function parameter annotations
        for func in &module.functions {
            for param in &func.parameters {
                let Some(ann_span) = param.annotation_span else {
                    continue;
                };
                let Some(ann) = source.get(ann_span.start as usize..ann_span.end as usize) else {
                    continue;
                };
                check_annotation(ann.trim(), param.name_span, path, diagnostics);
            }
        }
    }
}
