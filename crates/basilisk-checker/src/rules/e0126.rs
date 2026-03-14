//! BSK-E0126: `LiteralString` and `Literal` assignment incompatibilities.
//!
//! Detects annotated local variables inside function bodies where the declared
//! type is incompatible with the assigned value, specifically for `LiteralString`
//! and `Literal[...]` types.
//!
//! Covered cases:
//!
//! 1. Assigning a `Literal["X"]`-typed parameter to a `Literal["Y"]` variable
//!    where the literal values differ.
//! 2. Assigning an f-string containing non-`LiteralString` interpolations to
//!    a `LiteralString`-annotated variable.
//! 3. Assigning a generic parameterised with `str` where `LiteralString` is
//!    required (invariant generics like `list`, `Container`).
//! 4. Assigning a `list[LiteralString]` to `list[str]` — lists are invariant.
//!
//! ```python
//! def func(b: Literal["two"], non_literal: str):
//!     x1: Literal[""] = b                          # E — different literal values
//!     x2: LiteralString = f"{non_literal}"          # E — non-literal in f-string
//!     x3: Container[LiteralString] = Container(s)   # E — str ≠ LiteralString
//!     x4: list[str] = val                            # E — invariant mismatch
//! ```

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule};

use crate::diagnostic::Diagnostic;

use super::e0126_helpers::{
    emit_container_call_str_error, emit_fstring_literal_string_error,
    emit_invariant_container_mismatch, emit_literal_value_mismatch, extract_fstring_names,
    extract_literal_string_value, function_body_range, is_invariant_container, is_plain_str_type,
    is_simple_identifier, param_annotations, parse_annotated_assigns, parse_simple_call,
    split_generic, LocalAssign,
};
use super::Rule;

/// Emits BSK-E0126 for `LiteralString` / `Literal[...]` assignment
/// incompatibilities found inside function bodies.
pub(crate) struct LiteralStringAssignment;

impl Rule for LiteralStringAssignment {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        for func in &module.functions {
            check_function_body(func, source, path, diagnostics);
        }
    }
}

/// Check all annotated local assignments in a function body for
/// `LiteralString` / `Literal[...]` violations.
fn check_function_body(
    func: &FunctionInfo,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((body_start, body_end)) = function_body_range(func, source) else {
        return;
    };
    let Some(body) = source.get(body_start..body_end) else {
        return;
    };

    let param_anns = param_annotations(func, source);
    let assigns = parse_annotated_assigns(body, body_start);

    for assign in &assigns {
        check_literal_value_mismatch(assign, &param_anns, path, diagnostics);
        check_literal_string_fstring(assign, &param_anns, path, diagnostics);
        check_invariant_generic_literal_string(assign, &param_anns, path, diagnostics);
    }
}

/// Case 1: `x: Literal["X"] = param` where param is `Literal["Y"]` and X ≠ Y.
///
/// Also covers: `x: Literal[""] = param` where param is `Literal["two"]`.
fn check_literal_value_mismatch(
    assign: &LocalAssign<'_>,
    param_anns: &HashMap<&str, &str>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Annotation must be Literal["..."] (string literal).
    let Some(target_value) = extract_literal_string_value(assign.annotation) else {
        return;
    };

    // RHS must be a simple name referring to a parameter.
    let rhs_name = assign.rhs.trim();
    if !is_simple_identifier(rhs_name) {
        return;
    }

    let Some(param_ann) = param_anns.get(rhs_name) else {
        return;
    };

    // Parameter must also be Literal["..."].
    let Some(source_value) = extract_literal_string_value(param_ann) else {
        return;
    };

    if target_value != source_value {
        emit_literal_value_mismatch(assign, target_value, source_value, path, diagnostics);
    }
}

/// Case 2: `x: LiteralString = f"... {non_literal} ..."` where an
/// interpolated variable has type `str` (not `LiteralString`).
fn check_literal_string_fstring(
    assign: &LocalAssign<'_>,
    param_anns: &HashMap<&str, &str>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Annotation must be LiteralString.
    if assign.annotation.trim() != "LiteralString" {
        return;
    }

    // RHS must be an f-string.
    let rhs = assign.rhs.trim();
    if !rhs.starts_with("f\"") && !rhs.starts_with("f'") {
        return;
    }

    // Extract interpolated names from f-string: `{name}` patterns.
    let interpolated = extract_fstring_names(rhs);

    // If any interpolated name is a parameter with type `str` (not
    // `LiteralString` and not `Literal[...]`), emit a diagnostic.
    for name in &interpolated {
        if let Some(param_ann) = param_anns.get(name.as_str()) {
            if is_plain_str_type(param_ann) {
                emit_fstring_literal_string_error(assign, name, param_ann, path, diagnostics);
                return; // one diagnostic per assignment is enough
            }
        }
    }
}

/// Case 3 & 4: Invariant generic mismatches involving `LiteralString`.
///
/// - `x: Container[LiteralString] = Container(s)` where `s: str`
/// - `x: list[str] = val` where `val: list[LiteralString]`
fn check_invariant_generic_literal_string(
    assign: &LocalAssign<'_>,
    param_anns: &HashMap<&str, &str>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ann = assign.annotation.trim();
    let rhs = assign.rhs.trim();

    // Case 4: `list[str] = param` where param is `list[LiteralString]`.
    if let Some((ann_container, ann_inner)) = split_generic(ann) {
        if is_invariant_container(ann_container)
            && is_plain_str_type(ann_inner)
            && is_simple_identifier(rhs)
        {
            if let Some(param_ann) = param_anns.get(rhs) {
                if let Some((param_container, param_inner)) = split_generic(param_ann) {
                    if param_container == ann_container && param_inner.trim() == "LiteralString" {
                        emit_invariant_container_mismatch(
                            assign,
                            param_ann,
                            ann,
                            ann_container,
                            path,
                            diagnostics,
                        );
                        return;
                    }
                }
            }
        }
    }

    // Case 3: `Container[LiteralString] = Container(s)` where `s: str`.
    if let Some((_ann_container, ann_inner)) = split_generic(ann) {
        if ann_inner.trim() == "LiteralString" {
            if let Some((callee, call_args)) = parse_simple_call(rhs) {
                let _ = callee; // we only care about the args
                for arg in &call_args {
                    if let Some(param_ann) = param_anns.get(arg.as_str()) {
                        if is_plain_str_type(param_ann) {
                            emit_container_call_str_error(
                                assign,
                                rhs,
                                ann,
                                arg,
                                param_ann,
                                path,
                                diagnostics,
                            );
                            return;
                        }
                    }
                }
            }
        }
    }
}
