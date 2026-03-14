//! BSK-E0147: Tuple starred-unpack type compatibility violation.
//!
//! Detects assignments where a tuple literal or a tuple-typed variable is
//! assigned to a target whose annotation contains a starred unpack expression
//! (`*tuple[T, ...]` or `*tuple[T]`) and the assignment is incompatible with
//! that annotation.
//!
//! Covers module-level bare reassignments of annotated tuple variables and
//! function-body variable assignments.
//!
//! ## Examples
//!
//! ```python
//! t1: tuple[int, *tuple[str]] = (1, "")  # OK
//! t1 = (1, "", "")  # E — too many elements for *tuple[str]
//!
//! t2: tuple[int, *tuple[str, ...]] = (1, "")  # OK
//! t2 = (1, 1, "")  # E — second element must be str
//!
//! def f(t1: tuple[int], t2: tuple[int, *tuple[int, ...]], t3: tuple[int, ...]):
//!     v2: tuple[int, *tuple[int, ...]]
//!     v2 = t3  # E — homogeneous tuple[int,...] not assignable to mixed starred form
//!     v3: tuple[int]
//!     v3 = t2  # E — t2 may have more elements than v3 allows
//!     v3 = t3  # E — t3 is unbounded, v3 is fixed length 1
//! ```
//!
//! # Specification
//!
//! <https://typing.readthedocs.io/en/latest/spec/tuples.html#type-compatibility-rules>

pub(super) mod annotation;
pub(super) mod source;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

use annotation::{
    annotation_has_starred_unpack, check_literal_against_annotation, check_var_against_annotation,
    is_simple_name,
};
use source::{
    func_body_lines, iter_source_lines, line_span, make_diag, parse_annotated_decl,
    parse_bare_assignment, parse_tuple_literal,
};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0147",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0147",
};

/// Emits BSK-E0147 for incompatible starred-unpack tuple assignments.
pub(crate) struct TupleStarredUnpackCompatibility;

impl Rule for TupleStarredUnpackCompatibility {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        check_module_level(source, path, diagnostics);
        check_function_bodies(module, source, path, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Module-level bare reassignment checking
// ---------------------------------------------------------------------------

/// Check module-level bare assignments like `t2 = (1, 1, "")` after a
/// preceding annotated declaration like `t2: tuple[int, *tuple[str, ...]] = ...`.
fn check_module_level(source: &str, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    // Collect annotated module-level variables: name -> annotation text.
    let mut known_annotations: Vec<(String, String)> = Vec::new();

    for line_info in iter_source_lines(source) {
        let trimmed = line_info.text.trim();

        // Skip comment-only lines and blank lines.
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Annotated declaration: `name: type` or `name: type = value`
        if let Some((name, annotation)) = parse_annotated_decl(trimmed) {
            if annotation_has_starred_unpack(&annotation) {
                // Insert or update.
                if let Some(existing) = known_annotations.iter_mut().find(|(n, _)| n == &name) {
                    existing.1 = annotation;
                } else {
                    known_annotations.push((name, annotation));
                }
            }
            continue;
        }

        // Bare assignment: `name = (...)` — only module-level (not indented).
        if line_info.indent == 0 {
            if let Some((lhs, rhs)) = parse_bare_assignment(trimmed) {
                // Find previously declared annotation for this name.
                if let Some((_, annotation)) = known_annotations.iter().find(|(n, _)| n == &lhs) {
                    let annotation = annotation.clone();
                    // Only check when RHS is a tuple literal.
                    if let Some(elems) = parse_tuple_literal(rhs) {
                        if let Some(msg) = check_literal_against_annotation(&elems, &annotation) {
                            let span = line_span(source, line_info.offset);
                            diagnostics.push(make_diag(msg, span, path, &CODE));
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Function-body checking
// ---------------------------------------------------------------------------

/// Check inside function bodies for incompatible assignments to starred-unpack
/// annotated local variables, using parameter types as the source type.
fn check_function_bodies(
    module: &ResolvedModule,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for func in &module.functions {
        // Build a map: param_name -> annotation text.
        let mut param_annotations: Vec<(String, String)> = Vec::new();
        for param in &func.parameters {
            if let Some(ann_span) = param.annotation_span {
                if let Some(ann_text) = slice_span(source, ann_span) {
                    param_annotations.push((param.name.clone(), ann_text.trim().to_owned()));
                }
            }
        }

        // Collect local variable annotations declared inside the function.
        let mut local_annotations: Vec<(String, String)> = Vec::new();

        // Extract the function body source (lines indented past the `def`).
        let body_lines = func_body_lines(source, func.def_span.start_usize());

        for line_info in &body_lines {
            let trimmed = line_info.text.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Local annotated decl: `v2: tuple[int, *tuple[int, ...]]` or `v3: tuple[int]`.
            // Track all tuple annotations so we can check assignments where the source
            // type has a starred unpack but the target is a plain fixed-length tuple.
            if let Some((name, annotation)) = parse_annotated_decl(trimmed) {
                if annotation.starts_with("tuple[") {
                    if let Some(existing) = local_annotations.iter_mut().find(|(n, _)| n == &name) {
                        existing.1 = annotation;
                    } else {
                        local_annotations.push((name, annotation));
                    }
                }
                // Even if not a tuple annotation, continue — the annotated decl may
                // also carry a value (handled below as a normal assignment line).
            }

            // Bare assignment: `v2 = t3` inside the function body.
            if let Some((lhs, rhs)) = parse_bare_assignment(trimmed) {
                // Target must have a starred-unpack annotation.
                let target_ann = local_annotations
                    .iter()
                    .find(|(n, _)| n == &lhs)
                    .map(|(_, a)| a.clone());
                let Some(target_ann) = target_ann else {
                    continue;
                };

                let rhs = rhs.trim();

                // RHS is a simple name — look it up as a parameter annotation.
                if is_simple_name(rhs) {
                    if let Some((_, src_ann)) = param_annotations.iter().find(|(n, _)| n == rhs) {
                        let src_ann = src_ann.clone();
                        if let Some(msg) = check_var_against_annotation(&src_ann, &target_ann) {
                            let span = line_span(source, line_info.source_offset);
                            diagnostics.push(make_diag(msg, span, path, &CODE));
                        }
                    }
                    continue;
                }

                // RHS is a tuple literal.
                if let Some(elems) = parse_tuple_literal(rhs) {
                    if let Some(msg) = check_literal_against_annotation(&elems, &target_ann) {
                        let span = line_span(source, line_info.source_offset);
                        diagnostics.push(make_diag(msg, span, path, &CODE));
                    }
                }
            }
        }
    }
}
