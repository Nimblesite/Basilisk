//! Implements [`dataclasses_kwonly`] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-COERCION
//! `dataclasses_kwonly`: Dataclass constructor argument violations.
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

/// Emits `dataclasses_kwonly` for dataclass constructor argument violations:
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
// ##########################################################################
// # DELETED BODY — `build_positional_counts`. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
// #
// # `.bases.iter().filter_map(|base| class_map.get(base.as_str()))` summed inherited dataclass field counts by rendered base name.
// #
// # A base class's identity came from its RENDERED NAME, looked up in a map
// # keyed on `ClassInfo::name`. `ClassInfo::bases` is a `Vec<String>` the
// # resolver fills with "simple names only; complex expressions ignored", so:
// #   * a base reached through an alias  ->  MISSED
// #   * a dotted base (`httpx.Client`)   ->  collides with any local class
// #                                          sharing its trailing word
// #   * two classes with one rendered name -> a single map entry
// #
// # The replacement resolves each base EXPRESSION through the binding table
// # and keys the hierarchy on definition site. That needs base SPANS on
// # `ClassInfo`, which the resolver does not record yet.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
fn build_positional_counts(_classes: &[ClassInfo]) -> HashMap<&str, usize> {
    panic!(
        "basilisk-checker: `build_positional_counts` was DELETED because it identified base classes by \
         their RENDERED NAMES in a name-keyed map, so an aliased base missed and a \
         dotted base collided with any local class sharing its trailing word. It panics \
         because the real implementation — base expressions resolved through the binding \
         table — DOES NOT EXIST YET. Do not restore the name lookup and do not \
         substitute a default answer in its place."
    )
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
