//! Implements [dataclasses_kwonly] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-coercion
//! dataclasses_kwonly: Dataclass constructor argument violations.
//!
//! Reports errors when:
//! - A positional argument is passed to a keyword-only dataclass field
//! - A keyword argument targets a field with `init=False` (not part of `__init__`)
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

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ClassInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "dataclasses_kwonly",
    docs_url: "https://www.basilisk-python.dev/errors/dataclasses_kwonly",
};

/// Emits dataclasses_kwonly for dataclass constructor argument violations:
/// positional args to `kw_only` fields, and keyword args to `init=False` fields.
pub(crate) struct DataclassKwOnlyViolation;

impl Rule for DataclassKwOnlyViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let positional_counts = build_positional_counts(&module.classes);
        let init_false_fields = build_init_false_fields(&module.classes);

        let path = &module.path;
        for call in &module.calls {
            // Check positional argument limit (kw_only violation).
            if let Some(&positional_limit) = positional_counts.get(call.callee.as_str()) {
                if call.args.len() > positional_limit {
                    let extra = call.args.len() - positional_limit;
                    let span = call
                        .args
                        .get(positional_limit)
                        .map_or(call.span, |(_, s)| *s);
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "Too many positional arguments to `{}`: \
                             {extra} argument(s) must be passed as keyword arguments",
                            call.callee
                        ),
                        span,
                        path,
                        Some(format!(
                            "`{}` has keyword-only fields that cannot be passed positionally",
                            call.callee
                        )),
                        Some(
                            "Use keyword arguments for fields declared with `_: KW_ONLY`, \
                             `field(kw_only=True)`, or `@dataclass(kw_only=True)`"
                                .to_owned(),
                        ),
                    ));
                }
            }

            // Check keyword arguments targeting init=False fields.
            if let Some(no_init_names) = init_false_fields.get(call.callee.as_str()) {
                for (kw_name, _kw_kind) in &call.keywords {
                    if no_init_names.contains(kw_name.as_str()) {
                        diagnostics.push(error_diagnostic_owned(
                            CODE.clone(),
                            format!(
                                "Unexpected keyword argument `{kw_name}` for `{}`: \
                                 field `{kw_name}` is not included in `__init__`",
                                call.callee
                            ),
                            call.span,
                            path,
                            Some(format!(
                                "Field `{kw_name}` has `init=False` and cannot be passed \
                                 as a constructor argument"
                            )),
                            Some(
                                "Fields with `init=False` (or field specifiers that \
                                 implicitly set `init=False`) are excluded from `__init__`"
                                    .to_owned(),
                            ),
                        ));
                    }
                }
            }
        }
    }
}

/// Build a map from dataclass name -> number of positional (non-kw_only, non-init_false) fields,
/// including inherited positional fields from base dataclasses.
fn build_positional_counts(classes: &[ClassInfo]) -> HashMap<&str, usize> {
    let own_counts: HashMap<&str, usize> = classes
        .iter()
        .filter(|c| c.is_dataclass)
        .map(|c| {
            let own = c
                .attributes
                .iter()
                .filter(|a| a.has_annotation && !a.is_kw_only && !a.is_init_false)
                .count();
            (c.name.as_str(), own)
        })
        .collect();

    let class_map: HashMap<&str, &ClassInfo> =
        classes.iter().map(|c| (c.name.as_str(), c)).collect();

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
                .map(|base_class| {
                    own_counts
                        .get(base_class.name.as_str())
                        .copied()
                        .unwrap_or(0)
                })
                .sum();
            (c.name.as_str(), own + inherited)
        })
        .collect()
}

/// Build a map from dataclass name -> set of field names that have `init=False`.
fn build_init_false_fields(classes: &[ClassInfo]) -> HashMap<&str, HashSet<&str>> {
    classes
        .iter()
        .filter(|c| c.is_dataclass)
        .filter_map(|c| {
            let init_false: HashSet<&str> =
                basilisk_resolver::collect_name_set_where(&c.attributes, |a| a.is_init_false);
            if init_false.is_empty() {
                None
            } else {
                Some((c.name.as_str(), init_false))
            }
        })
        .collect()
}
