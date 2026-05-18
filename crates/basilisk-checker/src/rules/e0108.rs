//! BSK-E0108: Dataclass slots violations.
//!
//! Reports errors when:
//! - `self.attr = value` assigns to an attribute not in `__slots__` inside a
//!   class with `@dataclass(slots=True)` or a manual `__slots__` definition.
//! - `ClassName.__slots__` or `ClassName().__slots__` is accessed on a
//!   dataclass that does not define `__slots__` (neither via `slots=True` nor
//!   a manual `__slots__` assignment).
//!
//! ```python
//! @dataclass(slots=True)
//! class DC:
//!     x: int
//!     def __init__(self):
//!         self.y = 3  # E: "y" is not in __slots__
//!
//! @dataclass
//! class DC2:
//!     a: int
//! DC2.__slots__  # E: __slots__ not defined
//! ```

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ClassInfo, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, error_diagnostic_owned};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0108",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0108",
};

/// Emits BSK-E0108 for dataclass slots violations.
pub(crate) struct DataclassSlotsViolation;

impl Rule for DataclassSlotsViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        check_self_attr_assignments(module, diagnostics);
        check_slots_access_on_non_slots_class(module, diagnostics);
    }
}

/// Detect `self.attr = value` assignments inside methods of slot-constrained
/// classes where `attr` is not a declared field.
fn check_self_attr_assignments(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let source = &module.source;
    let path = &module.path;

    // Build a map of slot-constrained classes -> set of declared field names.
    let slot_classes: HashMap<&str, HashSet<&str>> = module
        .classes
        .iter()
        .filter(|c| c.is_dataclass_slots || (c.is_dataclass && c.has_manual_slots))
        .map(|c| {
            let fields: HashSet<&str> = c
                .attributes
                .iter()
                .filter(|a| a.has_annotation)
                .map(|a| a.name.as_str())
                .collect();
            (c.name.as_str(), fields)
        })
        .collect();

    if slot_classes.is_empty() {
        return;
    }

    // Walk each class body looking for method bodies with `self.ATTR = ...`
    // where ATTR is not a declared field.
    for cls in &module.classes {
        let Some(fields) = slot_classes.get(cls.name.as_str()) else {
            continue;
        };

        // Extract the class body source text.
        let Some(class_source) = slice_span(source, cls.def_span) else {
            continue;
        };
        let class_start = usize::try_from(cls.def_span.start).ok();
        let Some(class_start) = class_start else {
            continue;
        };

        // Find `self.ATTR = ` patterns in method bodies.
        find_undeclared_self_assignments(class_source, class_start, fields, cls, path, diagnostics);
    }
}

/// Scan class source text for `self.ATTR = value` patterns where ATTR is not
/// in the declared slots.
fn find_undeclared_self_assignments(
    class_source: &str,
    class_offset: usize,
    fields: &HashSet<&str>,
    cls: &ClassInfo,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Simple line-by-line scan for `self.ATTR = ` or `self.ATTR=` patterns.
    let mut pos: usize = 0;
    for line in class_source.lines() {
        let line_offset = pos;
        pos += line.len() + 1; // +1 for newline

        let trimmed = line.trim();

        // Match `self.ATTR = ...` assignment patterns.
        let Some(after_self) = trimmed.strip_prefix("self.") else {
            continue;
        };

        // Extract the attribute name (up to ` =`, `=`, `:`, or end of identifier).
        let attr_end = after_self
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(after_self.len());
        let Some(attr_name) = after_self.get(..attr_end) else {
            continue;
        };

        if attr_name.is_empty() {
            continue;
        }

        // Check this is an assignment (not just an access).
        let Some(rest) = after_self.get(attr_end..) else {
            continue;
        };
        let rest = rest.trim_start();
        if !rest.starts_with('=') || rest.starts_with("==") {
            continue;
        }

        // If attr_name is not in the declared fields, it is a violation.
        if !fields.contains(attr_name) {
            let byte_offset = class_offset + line_offset + (line.len() - trimmed.len());
            let span_end = byte_offset + "self.".len() + attr_name.len();
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Cannot assign to attribute `{attr_name}` on `{}`: \
                     `{attr_name}` is not defined in `__slots__`",
                    cls.name
                ),
                Span {
                    start: u32::try_from(byte_offset).unwrap_or(0),
                    end: u32::try_from(span_end).unwrap_or(0),
                },
                path,
                Some(format!(
                    "Only attributes declared in `__slots__` can be assigned; \
                     `{}` defines slots {fields_list}",
                    cls.name,
                    fields_list = format_fields(fields),
                )),
                Some(
                    "Slot-constrained classes (via `@dataclass(slots=True)` or manual \
                     `__slots__`) cannot have undeclared attributes"
                        .to_owned(),
                ),
            ));
        }
    }
}

/// Format field names for display in help messages.
fn format_fields(fields: &HashSet<&str>) -> String {
    let mut sorted: Vec<&str> = fields.iter().copied().collect();
    sorted.sort_unstable();
    format!(
        "[{}]",
        sorted
            .iter()
            .map(|f| format!("`{f}`"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Detect `ClassName.__slots__` or `ClassName(...)._slots__` access on a
/// dataclass that does not have slots defined.
fn check_slots_access_on_non_slots_class(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = &module.path;

    // Build a set of dataclass names that do NOT have slots.
    let non_slots_dataclasses: HashSet<&str> = module
        .classes
        .iter()
        .filter(|c| c.is_dataclass && !c.is_dataclass_slots && !c.has_manual_slots)
        .map(|c| c.name.as_str())
        .collect();

    if non_slots_dataclasses.is_empty() {
        return;
    }

    // Check module-level attribute accesses for `ClassName.__slots__`.
    for access in &module.module_attr_accesses {
        if access.attr_name != "__slots__" {
            continue;
        }
        if non_slots_dataclasses.contains(access.object_name.as_str()) {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Cannot access `__slots__` on `{}`: class does not define `__slots__`",
                    access.object_name
                ),
                access.span,
                path,
                Some(format!(
                    "Use `@dataclass(slots=True)` or define `__slots__` manually in `{}`",
                    access.object_name
                )),
                Some(
                    "Only classes with `@dataclass(slots=True)` or a manual `__slots__` \
                     definition have a `__slots__` attribute"
                        .to_owned(),
                ),
            ));
        }
    }

    // Also check source text for `ClassName(...)._slots__` patterns (instance access).
    check_instance_slots_access(module, &non_slots_dataclasses, diagnostics);
}

/// Detect `ClassName(...)._slots__` patterns in source text -- instance
/// construction followed by `.__slots__` attribute access.
fn check_instance_slots_access(
    module: &ResolvedModule,
    non_slots_dataclasses: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;

    // Compute byte offset of each line start for span calculation.
    let line_starts: Vec<usize> =
        std::iter::once(0)
            .chain(source.bytes().enumerate().filter_map(|(i, b)| {
                if b == b'\n' {
                    Some(i + 1)
                } else {
                    None
                }
            }))
            .collect();

    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        // Look for pattern: `ClassName(...)._slots__`
        for &class_name in non_slots_dataclasses {
            let pattern = format!("{class_name}(");
            let Some(call_pos) = trimmed.find(&pattern) else {
                continue;
            };

            // Find closing paren after the call.
            let Some(after_open) = trimmed.get(call_pos + pattern.len()..) else {
                continue;
            };
            let Some(close_paren) = find_matching_paren(after_open) else {
                continue;
            };

            let Some(after_close) = after_open.get(close_paren + 1..) else {
                continue;
            };
            if after_close.starts_with(".__slots__") {
                // Compute byte offset for the span.
                let Some(&line_start) = line_starts.get(line_idx) else {
                    continue;
                };
                let col_offset = line.len() - trimmed.len();
                let byte_start = line_start + col_offset + call_pos;
                let byte_end = byte_start + pattern.len() + close_paren + 1 + ".__slots__".len();
                let Some(span_start) = u32::try_from(byte_start).ok() else {
                    continue;
                };
                let Some(span_end) = u32::try_from(byte_end).ok() else {
                    continue;
                };

                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Cannot access `__slots__` on `{class_name}` instance: \
                         class does not define `__slots__`"
                    ),
                    Span {
                        start: span_start,
                        end: span_end,
                    },
                    path,
                    Some(format!(
                        "Use `@dataclass(slots=True)` or define `__slots__` manually in \
                         `{class_name}`"
                    )),
                    Some(
                        "Only classes with `@dataclass(slots=True)` or a manual `__slots__` \
                         definition have a `__slots__` attribute"
                            .to_owned(),
                    ),
                ));
            }
        }
    }
}

/// Find the position of the closing parenthesis matching an opening one,
/// handling nested parentheses.
fn find_matching_paren(text: &str) -> Option<usize> {
    let mut depth: usize = 1;
    for (idx, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
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
