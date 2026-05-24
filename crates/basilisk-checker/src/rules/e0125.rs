//! Implements [BSK-E0125] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! BSK-E0125: Access to instance attribute on a class object.
//!
//! Instance attributes (annotations without `ClassVar` in the class body that
//! lack a default value) exist only on instances, not on the class object
//! itself.  Accessing or assigning such attributes on the class (including
//! parameterised generics like `Node[int]`) is an error.
//!
//! ```python
//! from typing import Generic, TypeVar
//!
//! T = TypeVar("T")
//!
//! class Node(Generic[T]):
//!     label: T
//!
//! Node[int].label = 1  # E: instance attribute on class
//! Node[int].label      # E
//! Node.label = 1       # E
//! Node.label           # E
//! type(n1).label       # E
//! ```

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0125",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0125",
};

/// Emits BSK-E0125 for accessing instance-only attributes on class objects.
pub(crate) struct InstanceAttrOnClass;

/// Bundles shared context for the line-scanning helpers.
struct ScanContext<'a> {
    path: &'a str,
    class_instance_attrs: &'a HashMap<&'a str, HashSet<&'a str>>,
    var_class: &'a HashMap<&'a str, &'a str>,
}

impl Rule for InstanceAttrOnClass {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Build a map: class_name -> set of instance-only attribute names.
        // An instance-only attribute is one that has an annotation but no
        // default value AND is not wrapped in ClassVar.
        let class_instance_attrs: HashMap<&str, HashSet<&str>> = module
            .classes
            .iter()
            .filter(|cls| !cls.generic_params.is_empty())
            .filter_map(|cls| {
                let attrs: HashSet<&str> = cls
                    .attributes
                    .iter()
                    .filter(|attr| attr.has_annotation && !attr.has_value)
                    .filter(|attr| !is_classvar_annotation(source, attr.annotation_span))
                    .map(|attr| attr.name.as_str())
                    .collect();
                if attrs.is_empty() {
                    None
                } else {
                    Some((cls.name.as_str(), attrs))
                }
            })
            .collect();

        if class_instance_attrs.is_empty() {
            return;
        }

        // Build a set of all class names for quick lookup.
        let class_names: HashSet<&str> = class_instance_attrs.keys().copied().collect();

        // Build a map of variable name -> class name for type(var) patterns.
        let var_class: HashMap<&str, &str> = module
            .module_vars
            .iter()
            .filter_map(|var| {
                let rhs_span = var.rhs_span?;
                let rhs_text = slice_span(source, rhs_span)?;
                let callee = rhs_text.split(['(', '[']).next()?.trim();
                let callee = callee.rsplit('.').next().unwrap_or(callee);
                if class_names.contains(callee) {
                    Some((var.name.as_str(), callee))
                } else {
                    None
                }
            })
            .collect();

        // 1) Check module_attr_assignments for bare class name assignments
        //    (e.g. `Node.label = 1`).
        for assign in &module.module_attr_assignments {
            if let Some(attrs) = class_instance_attrs.get(assign.object_name.as_str()) {
                if attrs.contains(assign.attr_name.as_str()) {
                    diagnostics.push(make_diagnostic(
                        &assign.object_name,
                        &assign.attr_name,
                        assign.target_span,
                        path,
                    ));
                }
            }
        }

        // 2) Scan source lines for patterns not captured by the resolver.
        let ctx = ScanContext {
            path,
            class_instance_attrs: &class_instance_attrs,
            var_class: &var_class,
        };
        scan_source_lines(source, &ctx, diagnostics);
    }
}

/// Returns `true` when the annotation text starts with `ClassVar`.
fn is_classvar_annotation(source: &str, annotation_span: Option<Span>) -> bool {
    let Some(span) = annotation_span else {
        return false;
    };
    slice_span(source, span).is_some_and(|text| {
        let trimmed = text.trim();
        trimmed == "ClassVar" || trimmed.starts_with("ClassVar[")
    })
}

/// Scan the source text line-by-line for class-level instance attribute accesses.
fn scan_source_lines(source: &str, ctx: &ScanContext<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let mut byte_offset: u32 = 0;

    for line in source.lines() {
        let trimmed = line.trim();

        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            let leading_ws = u32::try_from(line.len() - line.trim_start().len()).unwrap_or(0);
            let line_start = byte_offset + leading_ws;

            check_subscript_class_attr_assign(trimmed, line_start, ctx, diagnostics);
            check_standalone_class_attr(trimmed, line_start, ctx, diagnostics);
            check_type_call_attr(trimmed, line_start, ctx, diagnostics);
        }

        // +1 for the newline character.
        byte_offset += u32::try_from(line.len()).unwrap_or(0) + 1;
    }
}

/// Detect `ClassName[...].attr = value` patterns (subscript assignment).
fn check_subscript_class_attr_assign(
    trimmed: &str,
    line_start: u32,
    ctx: &ScanContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (&class_name, attrs) in ctx.class_instance_attrs {
        let prefix = format!("{class_name}[");
        if !trimmed.starts_with(&prefix) {
            continue;
        }

        let rest = &trimmed[prefix.len()..];
        let Some(close_pos) = find_matching_bracket(rest) else {
            continue;
        };

        let after_close = &rest[close_pos + 1..];
        if !after_close.starts_with('.') {
            continue;
        }

        let after_dot = &after_close[1..];
        let attr_name = after_dot
            .split(|ch: char| ch.is_whitespace() || ch == '=')
            .next()
            .unwrap_or("");

        if attr_name.is_empty() {
            continue;
        }

        // Must be an assignment (has `=` but not `==` after attr).
        let after_attr = after_dot[attr_name.len()..].trim_start();
        if !after_attr.starts_with('=') || after_attr.starts_with("==") {
            continue;
        }

        if attrs.contains(attr_name) {
            let expr_len =
                u32::try_from(prefix.len() + close_pos + 1 + 1 + attr_name.len()).unwrap_or(0);
            diagnostics.push(make_diagnostic(
                class_name,
                attr_name,
                Span {
                    start: line_start,
                    end: line_start + expr_len,
                },
                ctx.path,
            ));
        }
    }
}

/// Detect standalone `ClassName.attr` or `ClassName[...].attr` expressions.
fn check_standalone_class_attr(
    trimmed: &str,
    line_start: u32,
    ctx: &ScanContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let stmt = trimmed.split('#').next().unwrap_or(trimmed).trim();

    for (&class_name, attrs) in ctx.class_instance_attrs {
        // Pattern 1: `ClassName.attr` (standalone, no assignment or call)
        let dot_prefix = format!("{class_name}.");
        if stmt.starts_with(&dot_prefix) {
            let after_dot = &stmt[dot_prefix.len()..];
            let attr_name = after_dot
                .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                .next()
                .unwrap_or("");
            let remainder = after_dot[attr_name.len()..].trim();
            if !attr_name.is_empty()
                && attrs.contains(attr_name)
                && remainder.is_empty()
                && !is_assignment_target(trimmed, &dot_prefix, attr_name)
            {
                let expr_len = u32::try_from(dot_prefix.len() + attr_name.len()).unwrap_or(0);
                diagnostics.push(make_diagnostic(
                    class_name,
                    attr_name,
                    Span {
                        start: line_start,
                        end: line_start + expr_len,
                    },
                    ctx.path,
                ));
            }
            continue;
        }

        // Pattern 2: `ClassName[...].attr` (standalone, no assignment)
        let bracket_prefix = format!("{class_name}[");
        if stmt.starts_with(&bracket_prefix) {
            let rest = &stmt[bracket_prefix.len()..];
            let Some(close_pos) = find_matching_bracket(rest) else {
                continue;
            };
            let after_close = &rest[close_pos + 1..];
            if !after_close.starts_with('.') {
                continue;
            }
            let after_dot = &after_close[1..];
            let attr_name = after_dot
                .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                .next()
                .unwrap_or("");
            let remainder = after_dot[attr_name.len()..].trim();
            if !attr_name.is_empty() && attrs.contains(attr_name) && remainder.is_empty() {
                let expr_len =
                    u32::try_from(bracket_prefix.len() + close_pos + 1 + 1 + attr_name.len())
                        .unwrap_or(0);
                diagnostics.push(make_diagnostic(
                    class_name,
                    attr_name,
                    Span {
                        start: line_start,
                        end: line_start + expr_len,
                    },
                    ctx.path,
                ));
            }
        }
    }
}

/// Detect `type(var).attr` standalone expressions.
fn check_type_call_attr(
    trimmed: &str,
    line_start: u32,
    ctx: &ScanContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let stmt = trimmed.split('#').next().unwrap_or(trimmed).trim();

    if !stmt.starts_with("type(") {
        return;
    }

    let after_type = &stmt["type(".len()..];
    let Some(close_paren) = after_type.find(')') else {
        return;
    };
    let var_name = after_type[..close_paren].trim();
    let after_paren = &after_type[close_paren + 1..];

    if !after_paren.starts_with('.') {
        return;
    }

    let after_dot = &after_paren[1..];
    let attr_name = after_dot
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .next()
        .unwrap_or("");
    let remainder = after_dot[attr_name.len()..].trim();

    if attr_name.is_empty() || !remainder.is_empty() {
        return;
    }

    let Some(&class_name) = ctx.var_class.get(var_name) else {
        return;
    };
    let Some(attrs) = ctx.class_instance_attrs.get(class_name) else {
        return;
    };

    if attrs.contains(attr_name) {
        let expr_len = u32::try_from(stmt.len()).unwrap_or(0);
        diagnostics.push(make_diagnostic(
            &format!("type({var_name})"),
            attr_name,
            Span {
                start: line_start,
                end: line_start + expr_len,
            },
            ctx.path,
        ));
    }
}

/// Find the position of the matching `]` for an opening `[`, handling nesting.
fn find_matching_bracket(text: &str) -> Option<usize> {
    let mut depth: u32 = 1;
    for (idx, ch) in text.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

/// Returns `true` when the trimmed line is an assignment to `ClassName.attr`.
fn is_assignment_target(trimmed: &str, dot_prefix: &str, attr_name: &str) -> bool {
    let target_len = dot_prefix.len() + attr_name.len();
    if trimmed.len() <= target_len {
        return false;
    }
    let after = trimmed[target_len..].trim_start();
    after.starts_with('=') && !after.starts_with("==")
}

/// Build a diagnostic for instance attribute access on a class object.
fn make_diagnostic(object_name: &str, attr_name: &str, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!("Cannot access instance attribute `{attr_name}` on class object `{object_name}`"),
        span,
        path,
        Some(format!(
            "`{attr_name}` is an instance attribute and can only be accessed on instances, \
             not on the class itself"
        )),
        Some(
            "Instance attributes (non-ClassVar annotations) exist only on instances. \
             Use an instance to access them, e.g. `Node[int]().label`"
                .to_owned(),
        ),
    )
}
