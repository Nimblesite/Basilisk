//! Implements [`typeddicts_inheritance`] from [CHKARCH-DIAG-OWNERSHIP] and
//! [CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE
//! `typeddicts_inheritance`: Invalid `TypedDict` inheritance.
//!
//! PEP 589 and the typing spec place constraints on `TypedDict` inheritance:
//!
//! 1. A `TypedDict` cannot inherit from both a `TypedDict` and a non-TypedDict
//!    base class (except `Generic`).
//!
//! 2. A `TypedDict` subclass cannot change the type of a field declared in a
//!    parent `TypedDict` class. PEP 705 refines this for the `ReadOnly`,
//!    `Required`, and `NotRequired` qualifiers:
//!    - A writable (non-`ReadOnly`) item may not be redeclared `ReadOnly`.
//!    - A required item may not be redeclared as not-required.
//!    - A writable item's value type is invariant; a `ReadOnly` item's value
//!      type may be narrowed to a subtype.
//!
//! 3. Multiple `TypedDict` inheritance is not allowed when two bases declare
//!    the same field with conflicting types or qualifiers.

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "typeddicts_inheritance",
    docs_url: "https://www.basilisk-python.dev/errors/typeddicts_inheritance",
};

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic(
        CODE.clone(),
        message,
        span,
        path,
        None,
        Some("PEP 589: TypedDict subclassing has strict field-compatibility requirements"),
    )
}

/// Checks rule 1: `TypedDict` cannot mix `TypedDict` and non-TypedDict bases.
fn check_mixed_bases(
    cls: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    const EXEMPT: &[&str] = &["object"];
    let has_typed_dict_base = cls
        .bases
        .iter()
        .any(|b| basilisk_resolver::is_transitive_typeddict(b.as_str(), class_map));
    if !has_typed_dict_base {
        return;
    }
    for base in &cls.bases {
        if EXEMPT.contains(&base.as_str()) {
            continue;
        }
        if !basilisk_resolver::is_transitive_typeddict(base.as_str(), class_map) {
            diagnostics.push(make_diagnostic(
                format!(
                    "TypedDict `{}` cannot inherit from non-TypedDict class `{}`",
                    cls.name, base
                ),
                cls.name_span,
                path,
            ));
        }
    }
}

/// Emits `typeddicts_inheritance` for invalid `TypedDict` inheritance.
pub(crate) struct InvalidTypedDictInheritance;

impl Rule for InvalidTypedDictInheritance {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let class_map = super::shared::class_name_map(&module.classes);

        for cls in &module.classes {
            if !basilisk_resolver::is_transitive_typeddict(cls.name.as_str(), &class_map) {
                continue;
            }

            check_mixed_bases(cls, &class_map, &module.path, diagnostics);
        }
    }
}
