//! BSK-E0060: Invalid ordering comparison of dataclass instances.
//!
//! When `@dataclass(order=True)`, Python synthesizes `__lt__`, `__le__`, `__gt__`,
//! and `__ge__` methods.  These methods raise `TypeError` at runtime if the other
//! operand is not an instance of the **same** class.  Comparing two `order=True`
//! dataclass instances of different types with `<`, `<=`, `>`, or `>=` is therefore
//! a type error.
//!
//! Additionally, when a class does NOT have `order=True` (including
//! `dataclass_transform` classes with `order=False`), ordering comparisons are
//! not supported at all because `__lt__` etc. are never synthesized.
//!
//! ```python
//! from dataclasses import dataclass
//!
//! @dataclass(order=True)
//! class DC1:
//!     a: str
//!
//! @dataclass(order=True)
//! class DC2:
//!     a: str
//!
//! dc1 = DC1("x")
//! dc2 = DC2("y")
//!
//! if dc1 < dc2:   # E: incompatible types
//!     pass
//! ```

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0060",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0060",
};

/// A comparison extracted from source text.
struct SourceComparison {
    left_name: String,
    right_name: String,
    span: Span,
}

/// Emits BSK-E0060 for invalid ordering comparisons of dataclass instances:
/// - Cross-type comparisons between different `order=True` dataclass classes.
/// - Any ordering comparison on dataclass instances where `order` is not enabled.
pub(crate) struct CrossTypeDataclassOrderComparison;

impl Rule for CrossTypeDataclassOrderComparison {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let transform_classes = super::guards::collect_transform_classes(module);

        let all_dc_classes = collect_dc_classes(module, &transform_classes);
        if all_dc_classes.is_empty() {
            return;
        }

        let order_classes: HashSet<&str> = all_dc_classes
            .iter()
            .filter(|(_, &has_order)| has_order)
            .map(|(&name, _)| name)
            .collect();

        let var_class = build_var_class_map(module, &all_dc_classes);
        if var_class.is_empty() {
            return;
        }

        let comparisons = gather_comparisons(module);
        emit_comparison_diagnostics(
            &comparisons,
            &var_class,
            &order_classes,
            module,
            diagnostics,
        );
    }
}

/// Collects all dataclass-like classes and whether they have order enabled.
fn collect_dc_classes<'a>(
    module: &'a ResolvedModule,
    transform_classes: &HashMap<String, super::guards::TransformClassInfo>,
) -> HashMap<&'a str, bool> {
    let mut result: HashMap<&str, bool> = HashMap::new();
    for cls in &module.classes {
        if cls.is_dataclass {
            result.insert(cls.name.as_str(), cls.is_dataclass_order);
        } else if let Some(info) = transform_classes.get(cls.name.as_str()) {
            result.insert(cls.name.as_str(), info.order);
        }
    }
    result
}

/// Builds a map from variable name to the dataclass it was instantiated from.
fn build_var_class_map<'a>(
    module: &'a ResolvedModule,
    all_dc_classes: &HashMap<&str, bool>,
) -> HashMap<&'a str, &'a str> {
    module
        .module_vars
        .iter()
        .filter_map(|var| {
            let rhs_span = var.rhs_span?;
            let rhs_text = module
                .source
                .get(rhs_span.start as usize..rhs_span.end as usize)?;
            let callee = rhs_text.split(['(', '[']).next()?.trim();
            let callee = callee.rsplit('.').next().unwrap_or(callee);
            if all_dc_classes.contains_key(callee) {
                Some((var.name.as_str(), callee))
            } else {
                None
            }
        })
        .collect()
}

/// Gathers comparisons from the resolver or extracts them from source text.
fn gather_comparisons(module: &ResolvedModule) -> Vec<SourceComparison> {
    if module.module_order_comparisons.is_empty() {
        extract_comparisons_from_source(&module.source)
    } else {
        module
            .module_order_comparisons
            .iter()
            .map(|cmp| SourceComparison {
                left_name: cmp.left_name.clone(),
                right_name: cmp.right_name.clone(),
                span: cmp.span,
            })
            .collect()
    }
}

/// Emits diagnostics for invalid ordering comparisons.
fn emit_comparison_diagnostics(
    comparisons: &[SourceComparison],
    var_class: &HashMap<&str, &str>,
    order_classes: &HashSet<&str>,
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for cmp in comparisons {
        let Some(&left_class) = var_class.get(cmp.left_name.as_str()) else {
            continue;
        };
        let Some(&right_class) = var_class.get(cmp.right_name.as_str()) else {
            continue;
        };

        let left_has_order = order_classes.contains(left_class);
        let right_has_order = order_classes.contains(right_class);

        if !left_has_order || !right_has_order {
            let no_order_class = if left_has_order {
                right_class
            } else {
                left_class
            };
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Cannot use ordering operator on `{no_order_class}`: \
                     comparison methods are not synthesized (order is not enabled)"
                ),
                span: cmp.span,
                path: module.path.clone(),
                help: Some(
                    "Enable `order=True` on the dataclass to synthesize ordering methods"
                        .to_owned(),
                ),
                note: Some(
                    "Without `order=True`, `__lt__`, `__le__`, `__gt__`, `__ge__` \
                     are not generated"
                        .to_owned(),
                ),
            });
        } else if left_class != right_class {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Cannot compare `{left_class}` and `{right_class}` with ordering operator: \
                     `@dataclass(order=True)` comparison methods only accept the same type"
                ),
                span: cmp.span,
                path: module.path.clone(),
                help: Some(
                    "Ordering comparisons (`<`, `<=`, `>`, `>=`) between different dataclass \
                     types are not supported"
                        .to_owned(),
                ),
                note: Some(
                    "PEP 557: the synthesized `__lt__` etc. return `NotImplemented` for \
                     instances of a different type"
                        .to_owned(),
                ),
            });
        }
    }
}

/// Extracts ordering comparisons from module source text.
///
/// Scans for patterns like `name1 < name2`, `name1 <= name2`, `name1 > name2`,
/// `name1 >= name2` in module-level code lines (not inside class/function bodies).
#[allow(clippy::cast_possible_truncation)]
fn extract_comparisons_from_source(source: &str) -> Vec<SourceComparison> {
    let mut results = Vec::new();
    let operators = [" <= ", " >= ", " < ", " > "];

    let mut byte_offset: u32 = 0;
    for line in source.lines() {
        let trimmed = line.trim();

        // Skip comments, class/function defs, and decorators
        let dominated = trimmed.starts_with('#')
            || trimmed.starts_with("class ")
            || trimmed.starts_with("def ")
            || trimmed.starts_with('@');

        if !dominated {
            for op in &operators {
                if let Some(op_pos) = line.find(op) {
                    let left_part = line[..op_pos].trim();
                    let left_name = extract_last_identifier(left_part);

                    let right_part = line[op_pos + op.len()..].trim();
                    let right_name = extract_first_identifier(right_part);

                    if let (Some(left), Some(right)) = (left_name, right_name) {
                        results.push(SourceComparison {
                            left_name: left.to_owned(),
                            right_name: right.to_owned(),
                            span: Span {
                                start: byte_offset,
                                end: byte_offset + line.len() as u32,
                            },
                        });
                        break; // Only match first operator per line
                    }
                }
            }
        }

        byte_offset += line.len() as u32 + 1; // +1 for newline
    }

    results
}

/// Extracts the last Python identifier from a string fragment.
fn extract_last_identifier(text: &str) -> Option<&str> {
    let text = text.trim();
    // Walk backwards to find the identifier
    let end = text.len();
    let start = text
        .bytes()
        .enumerate()
        .rev()
        .find(|(_, byte)| !byte.is_ascii_alphanumeric() && *byte != b'_')
        .map_or(0, |(idx, _)| idx + 1);

    let ident = &text[start..end];
    if ident.is_empty() || ident.bytes().next()?.is_ascii_digit() {
        None
    } else {
        Some(ident)
    }
}

/// Extracts the first Python identifier from a string fragment.
fn extract_first_identifier(text: &str) -> Option<&str> {
    let text = text.trim();
    let end = text
        .bytes()
        .position(|byte| !byte.is_ascii_alphanumeric() && byte != b'_')
        .unwrap_or(text.len());

    let ident = &text[..end];
    if ident.is_empty() || ident.bytes().next()?.is_ascii_digit() {
        None
    } else {
        Some(ident)
    }
}
