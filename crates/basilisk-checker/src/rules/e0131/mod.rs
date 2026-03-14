//! BSK-E0131: Generator yield/send/return type mismatch.
//!
//! When a function is annotated with `Generator[Y, S, R]`, `Iterator[Y]`,
//! or `Iterable[Y]`, the yield expressions must produce values compatible
//! with `Y`, and `yield from` expressions must delegate to generators whose
//! yield and send types are compatible.
//!
//! ```python
//! from typing import Generator, Iterator
//!
//! class A: ...
//! class B: ...
//!
//! def bad() -> Generator[A, None, None]:
//!     yield 3          # E: incompatible yield type
//!
//! def bad2() -> Iterator[A]:
//!     yield B()        # E: incompatible yield type
//! ```

mod annotation;
mod type_check;
mod yield_scan;

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule, ReturnAnnotationKind};

use crate::diagnostic::{Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

use annotation::parse_generator_annotation;
use type_check::{
    check_missing_generator_return, check_yield_from, check_yield_value,
};
use yield_scan::find_yield_expressions;

pub(crate) const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0131",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0131",
};

/// Emits BSK-E0131 for generator yield/send/return type mismatches.
pub(crate) struct GeneratorTypeMismatch;

impl Rule for GeneratorTypeMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build a map of function name -> return annotation text for cross-referencing
        // yield-from targets.
        let func_return_annotations: HashMap<&str, &str> = module
            .functions
            .iter()
            .filter_map(|func| {
                let ann_span = func.return_annotation_span?;
                let ann_text = slice_span(&module.source, ann_span)?;
                Some((func.name.as_str(), ann_text.trim()))
            })
            .collect();

        for func in &module.functions {
            check_function(
                func,
                &module.source,
                &module.path,
                &func_return_annotations,
                diagnostics,
            );
        }
    }
}

/// Check a single function for generator type mismatches.
fn check_function(
    func: &FunctionInfo,
    source: &str,
    path: &str,
    func_return_annotations: &HashMap<&str, &str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if func.return_annotation == ReturnAnnotationKind::Missing {
        return;
    }

    let Some(ann_span) = func.return_annotation_span else {
        return;
    };
    let Some(ann_text) = slice_span(source, ann_span) else {
        return;
    };
    let ann_text = ann_text.trim();

    let Some(gen_ann) = parse_generator_annotation(ann_text) else {
        return;
    };

    // Find the function body.
    let def_start = func.def_span.start_usize();
    let Some(colon_rel) = source.get(def_start..).and_then(|s| s.find(':')) else {
        return;
    };
    let body_start = def_start + colon_rel + 1;

    let def_line_start = source
        .get(..def_start)
        .and_then(|s| s.rfind('\n'))
        .map_or(0, |idx| idx + 1);
    let def_indent = def_start - def_line_start;

    let body_end = find_body_end(source, body_start, def_indent);
    let Some(body) = source.get(body_start..body_end) else {
        return;
    };

    let yields = find_yield_expressions(body, body_start);

    for yield_expr in &yields {
        if yield_expr.is_yield_from {
            check_yield_from(
                yield_expr,
                &gen_ann,
                func,
                path,
                func_return_annotations,
                source,
                diagnostics,
            );
        } else {
            check_yield_value(yield_expr, &gen_ann, func, path, diagnostics);
        }
    }

    check_missing_generator_return(func, &gen_ann, &yields, path, diagnostics);
}

/// Find the end byte offset of a function body in the source.
fn find_body_end(source: &str, body_start: usize, def_indent: usize) -> usize {
    let mut pos = body_start;
    let mut first_line = true;

    for line in source.get(body_start..).unwrap_or("").lines() {
        if first_line {
            first_line = false;
            pos += line.len() + 1;
            continue;
        }

        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            pos += line.len() + 1;
            continue;
        }

        let line_indent = line.len() - trimmed.len();
        if line_indent <= def_indent
            && (trimmed.starts_with("def ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("async def ")
                || trimmed.starts_with('@'))
        {
            return pos;
        }
        pos += line.len() + 1;
    }

    source.len()
}
