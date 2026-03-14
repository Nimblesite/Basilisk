//! BSK-E0096: Type mismatch between a dataclass `field(default_factory=…)` and
//! the field's declared type annotation.
//!
//! When a dataclass field uses `field(default_factory=T)` where `T` is a known
//! callable that constructs instances of a simple built-in type, but the field's
//! annotation declares a different incompatible built-in type, Basilisk reports
//! an error.
//!
//! ```python
//! from dataclasses import dataclass, field
//!
//! @dataclass
//! class DC:
//!     a: int = field(default_factory=str)  # E: str() → str, not int
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0096",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0096",
};

/// Emits BSK-E0096 for dataclass fields whose `default_factory` type is
/// incompatible with the declared field annotation.
pub(crate) struct DataclassFieldDefaultFactoryMismatch;

impl Rule for DataclassFieldDefaultFactoryMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        for cls in &module.classes {
            if !cls.is_dataclass {
                continue;
            }
            for attr in &cls.attributes {
                if !attr.has_value {
                    continue;
                }
                let Some(ann_span) = attr.annotation_span else {
                    continue;
                };
                let Some(rhs_span) = attr.rhs_span else {
                    continue;
                };
                let Some(ann_text) = slice_span(source, ann_span) else {
                    continue;
                };
                let Some(rhs_text) = slice_span(source, rhs_span) else {
                    continue;
                };

                let Some(factory_type) = extract_default_factory_type(rhs_text) else {
                    continue;
                };

                if is_factory_incompatible_with_annotation(factory_type, ann_text.trim()) {
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Field `{}` is annotated as `{}` but \
                             `default_factory={factory_type}` produces `{factory_type}` values",
                            attr.name,
                            ann_text.trim(),
                        ),
                        span: attr.name_span,
                        path: path.clone(),
                        help: Some(format!(
                            "Change the annotation to `{factory_type}` or use a compatible \
                             `default_factory`"
                        )),
                        note: Some(
                            "PEP 557: `default_factory` must be a zero-argument callable \
                             compatible with the field's declared type"
                                .to_owned(),
                        ),
                    });
                }
            }
        }
    }
}

/// Extract the type name from `field(default_factory=TypeName)`.
///
/// Returns `Some("TypeName")` for `field(default_factory=str)`,
/// `field(default_factory=int)`, etc.  Returns `None` for anything more
/// complex (lambdas, attribute accesses, calls, etc.).
fn extract_default_factory_type(rhs_text: &str) -> Option<&str> {
    let inner = rhs_text.trim().strip_prefix("field(")?.trim_start();

    // Find "default_factory="
    let factory_idx = inner.find("default_factory=")?;
    let after = inner.get(factory_idx + "default_factory=".len()..)?;

    // Extract value until the first comma or closing paren.
    let end = after
        .find(',')
        .or_else(|| after.find(')'))
        .unwrap_or(after.len());
    let factory_val = after.get(..end)?.trim();

    // Only accept simple identifiers (no dots, brackets, parens, spaces).
    if factory_val.is_empty()
        || factory_val
            .chars()
            .any(|c| !c.is_alphanumeric() && c != '_')
    {
        return None;
    }

    Some(factory_val)
}

/// Returns `true` when calling `factory_type()` yields a value that is
/// clearly incompatible with `field_ann`.
///
/// Only flags mismatches between well-known primitive types to avoid FPs.
fn is_factory_incompatible_with_annotation(factory_type: &str, field_ann: &str) -> bool {
    // Strip away Optional/Union wrappers — just look at the primary type name.
    let primary = field_ann.split('|').next().unwrap_or(field_ann).trim();
    // A factory that is not a known primitive → don't flag.
    let known = ["str", "int", "float", "bool", "bytes"];
    if !known.contains(&factory_type) {
        return false;
    }
    if !known.contains(&primary) {
        return false;
    }
    // str, bytes are their own types; int/bool/float share the numeric tower.
    match factory_type {
        "str" => primary != "str",
        "bytes" => primary != "bytes",
        "int" => matches!(primary, "str" | "bytes"),
        "float" => matches!(primary, "str" | "bytes"),
        "bool" => matches!(primary, "str" | "bytes"),
        _ => false,
    }
}
