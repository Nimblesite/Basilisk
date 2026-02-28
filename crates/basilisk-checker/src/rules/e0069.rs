//! BSK-E0069: Positional argument passed to a keyword-only dataclass field.
//!
//! When a dataclass field is keyword-only (via `_: KW_ONLY` sentinel,
//! `field(kw_only=True)`, or `@dataclass(kw_only=True)`), it cannot be
//! passed as a positional argument at the call site.
//!
//! ```python
//! from dataclasses import dataclass, KW_ONLY
//!
//! @dataclass
//! class Point:
//!     x: float
//!     _: KW_ONLY
//!     y: float = 0.0
//!
//! Point(1.0)       # OK — x positional, y uses default
//! Point(1.0, 2.0)  # E — y is keyword-only, cannot be passed positionally
//! ```

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0069",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0069",
};

/// Emits BSK-E0069 when a positional argument is passed to a keyword-only
/// dataclass field.
pub(crate) struct DataclassKwOnlyViolation;

impl Rule for DataclassKwOnlyViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build a map from dataclass name → number of positional (non-kw_only) fields.
        // Inheritance: each base-class positional field count is added to the subclass.
        let positional_counts = build_positional_counts(&module.classes);

        let path = &module.path;
        for call in &module.calls {
            let Some(&positional_limit) = positional_counts.get(call.callee.as_str()) else {
                continue;
            };
            if call.args.len() > positional_limit {
                let extra = call.args.len() - positional_limit;
                // Span the first extra positional argument.
                let span = call
                    .args
                    .get(positional_limit)
                    .map_or(call.span, |(_, s)| *s);
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Too many positional arguments to `{}`: \
                         {extra} argument(s) must be passed as keyword arguments",
                        call.callee
                    ),
                    span,
                    path: path.clone(),
                    help: Some(format!(
                        "`{}` has keyword-only fields that cannot be passed positionally",
                        call.callee
                    )),
                    note: Some(
                        "Use keyword arguments for fields declared with `_: KW_ONLY`, \
                         `field(kw_only=True)`, or `@dataclass(kw_only=True)`"
                            .to_owned(),
                    ),
                });
            }
        }
    }
}

/// Build a map from dataclass name → number of positional (non-kw_only) fields,
/// including inherited positional fields from base dataclasses.
fn build_positional_counts(classes: &[ClassInfo]) -> HashMap<&str, usize> {
    // First pass: own positional counts (not accounting for inheritance).
    let own_counts: HashMap<&str, usize> = classes
        .iter()
        .filter(|c| c.is_dataclass)
        .map(|c| {
            let own = c
                .attributes
                .iter()
                .filter(|a| a.has_annotation && !a.is_kw_only)
                .count();
            (c.name.as_str(), own)
        })
        .collect();

    // Build a map of class name → ClassInfo for inheritance lookup.
    let class_map: HashMap<&str, &ClassInfo> = classes
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect();

    // Second pass: for each dataclass, add positional counts from base dataclasses.
    classes
        .iter()
        .filter(|c| c.is_dataclass)
        .map(|c| {
            let own = own_counts.get(c.name.as_str()).copied().unwrap_or(0);
            let inherited: usize = c
                .bases
                .iter()
                .filter_map(|base| class_map.get(base.as_str()))
                .filter(|base_class| base_class.is_dataclass)
                .map(|base_class| own_counts.get(base_class.name.as_str()).copied().unwrap_or(0))
                .sum();
            (c.name.as_str(), own + inherited)
        })
        .collect()
}
