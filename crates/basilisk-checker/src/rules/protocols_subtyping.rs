//! Implements [BSK-E0124] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! BSK-E0124: Protocol attribute tuple element type mismatch.
//!
//! When a class explicitly implements a `Protocol` and assigns to a
//! `self.attr` in `__init__` where `attr` is declared as `tuple[T1, T2, ...]`
//! in the protocol, each element of the assigned tuple must have a compatible
//! type.  If a parameter used in the tuple has a different type than the
//! corresponding element type in the protocol's annotation, Basilisk reports
//! the mismatch.
//!
//! ```python
//! from typing import Protocol
//!
//! class RGB(Protocol):
//!     rgb: tuple[int, int, int]
//!
//! class Point(RGB):
//!     def __init__(self, red: int, green: int, blue: str) -> None:
//!         self.rgb = red, green, blue  # E — 'blue' must be 'int'
//! ```

use std::collections::HashMap;

use basilisk_resolver::{AttributeInfo, ClassInfo, FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

use crate::rules::shared::is_type_compatible;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0124",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0124",
};

/// Emits BSK-E0124 when a tuple assignment to a protocol attribute has
/// element types that don't match the protocol's declaration.
pub(crate) struct ProtocolTupleElementMismatch;

impl Rule for ProtocolTupleElementMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let class_map = super::shared::class_name_map(&module.classes);

        let method_map: HashMap<(&str, &str), &FunctionInfo> = module
            .functions
            .iter()
            .filter_map(|func| {
                func.class_name
                    .as_deref()
                    .map(|cls| ((cls, func.name.as_str()), func))
            })
            .collect();

        for class in &module.classes {
            check_class(
                class,
                &class_map,
                &method_map,
                &module.source,
                &module.path,
                diagnostics,
            );
        }
    }
}

/// Check a class's `__init__` for tuple assignments to protocol attributes.
fn check_class(
    class: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
    method_map: &HashMap<(&str, &str), &FunctionInfo>,
    source: &str,
    path: &str,
    out: &mut Vec<Diagnostic>,
) {
    // Find protocol bases.
    let protocol_bases: Vec<&ClassInfo> = class
        .bases
        .iter()
        .filter_map(|base_name| class_map.get(base_name.as_str()).copied())
        .filter(|base| is_protocol_class(base))
        .collect();

    if protocol_bases.is_empty() {
        return;
    }

    // Get the __init__ method.
    let Some(init_func) = method_map.get(&(class.name.as_str(), "__init__")) else {
        return;
    };

    // Build parameter type map (param_name -> annotation text).
    let param_types: HashMap<&str, &str> = init_func
        .parameters
        .iter()
        .filter_map(|param| {
            let ann_span = param.annotation_span?;
            let ann_text = slice_span(source, ann_span)?;
            Some((param.name.as_str(), ann_text))
        })
        .collect();

    // Scan the function body source text for `self.attr = expr` patterns.
    let Some(func_source) = slice_span(source, init_func.def_span) else {
        return;
    };
    let func_offset = usize::try_from(init_func.def_span.start).unwrap_or(0);

    for line in func_source.lines() {
        let trimmed = line.trim();

        // Look for `self.X = expr` pattern.
        let Some(attr_assignment) = parse_self_attr_assignment(trimmed) else {
            continue;
        };

        let attr_name = attr_assignment.attr_name;
        let rhs_text = attr_assignment.rhs;

        // Check if this attribute is declared in a protocol base with a tuple type.
        for protocol_base in &protocol_bases {
            let Some(attr_info) = find_attribute(protocol_base, attr_name) else {
                continue;
            };

            let Some(ann_span) = attr_info.annotation_span else {
                continue;
            };

            let Some(ann_text) = slice_span(source, ann_span) else {
                continue;
            };

            // Parse the annotation as tuple[T1, T2, ...].
            let Some(element_types) = parse_tuple_annotation(ann_text) else {
                continue;
            };

            // Parse the RHS as comma-separated values.
            let rhs_values: Vec<&str> = rhs_text.split(',').map(str::trim).collect();

            if rhs_values.len() != element_types.len() {
                continue;
            }

            // Check each value against the expected element type.
            for (idx, (value, expected_type)) in
                rhs_values.iter().zip(element_types.iter()).enumerate()
            {
                // If the value is a parameter name, check its declared type.
                let Some(param_type) = param_types.get(value) else {
                    continue;
                };

                if !is_type_compatible(param_type, expected_type) {
                    // Find the span for this line in the source.
                    let line_offset = find_line_offset(func_source, line);
                    let absolute_offset = func_offset + line_offset;
                    let span = Span {
                        start: u32::try_from(absolute_offset).unwrap_or(0),
                        end: u32::try_from(absolute_offset + line.len()).unwrap_or(0),
                    };

                    out.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "Parameter `{value}` has type `{param_type}` but element {idx} of \
                             `{attr_name}` in protocol `{}` expects `{expected_type}`",
                            protocol_base.name
                        ),
                        span,
                        path,
                        Some(format!(
                            "Change parameter `{value}` to type `{expected_type}` or convert it \
                             before assigning to `self.{attr_name}`"
                        )),
                        Some(format!(
                            "Protocol `{}` declares `{attr_name}: {ann_text}` — all tuple \
                             elements must match the declared types",
                            protocol_base.name
                        )),
                    ));
                }
            }
        }
    }
}

/// Parsed `self.attr = rhs` assignment.
struct SelfAttrAssignment<'a> {
    attr_name: &'a str,
    rhs: &'a str,
}

/// Parse a line like `self.attr = expr` into its components.
fn parse_self_attr_assignment(line: &str) -> Option<SelfAttrAssignment<'_>> {
    let line = line.strip_prefix("self.")?;

    let eq_idx = line.find('=')?;

    // Make sure it's `=` not `==`.
    if line.get(eq_idx + 1..eq_idx + 2) == Some("=") {
        return None;
    }

    let attr_name = line.get(..eq_idx)?.trim();
    let rhs = line.get(eq_idx + 1..)?.trim();

    // Strip trailing comments.
    let rhs = rhs.split('#').next().map(str::trim)?;

    if attr_name.is_empty() || rhs.is_empty() {
        return None;
    }

    // Attribute name must be a simple identifier.
    if !attr_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }

    Some(SelfAttrAssignment { attr_name, rhs })
}

/// Parse a `tuple[T1, T2, ...]` annotation into its element types.
fn parse_tuple_annotation(ann: &str) -> Option<Vec<&str>> {
    let inner = ann.strip_prefix("tuple[")?;
    let inner = inner.strip_suffix(']')?;

    let elements: Vec<&str> = inner.split(',').map(str::trim).collect();
    if elements.is_empty() {
        return None;
    }

    Some(elements)
}

/// Returns `true` when the class directly lists `Protocol` among its bases.
fn is_protocol_class(class: &ClassInfo) -> bool {
    class.bases.iter().any(|base| base == "Protocol")
}

/// Find an attribute by name in a class.
fn find_attribute<'a>(class: &'a ClassInfo, name: &str) -> Option<&'a AttributeInfo> {
    class.attributes.iter().find(|attr| attr.name == name)
}

/// Find the byte offset of a line within a larger text.
///
/// Uses the fact that `line` is a sub-slice of `text` (produced by
/// `str::lines()`), so we can compute the offset via pointer addresses
/// without `as` conversions.
fn find_line_offset(text: &str, line: &str) -> usize {
    line.as_ptr().addr().saturating_sub(text.as_ptr().addr())
}
