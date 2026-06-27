//! Implements [BSK-E0157] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-ownership
//! BSK-E0157: Dataclass field without a default after a field with a default.
//!
//! A dataclass synthesizes an `__init__` whose parameters follow field
//! declaration order. A field *without* a default that follows a field *with* a
//! default would produce a non-default argument after a default one — a
//! `TypeError` at class-definition time. `field(default=...)` and `InitVar`
//! fields with a value both count as "has a default"; `ClassVar`, `kw_only`, and
//! `field(init=False)` fields are excluded because they do not become positional
//! `__init__` parameters.
//!
//! ```python
//! @dataclass
//! class C:
//!     a: int = 0
//!     b: int  # E0157: no-default field after a defaulted one
//! ```

use basilisk_resolver::{AttributeInfo, ClassInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::shared::annotation_is_classvar;
use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0157",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0157",
};

/// Emits BSK-E0157 for a non-default dataclass field that follows a defaulted one.
pub(crate) struct DataclassFieldOrder;

impl Rule for DataclassFieldOrder {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for class in &module.classes {
            // Inheritance reorders fields through the MRO; only check standalone
            // dataclasses, mirroring the conservatism of E0041.
            if class.is_dataclass && class.bases.is_empty() {
                check_class(class, &module.source, &module.path, diagnostics);
            }
        }
    }
}

/// Returns `true` when `attr` is a positional `__init__` field (so it
/// participates in default-ordering). `InitVar` fields are included — they DO
/// become `__init__` parameters.
fn is_positional_init_field(attr: &AttributeInfo, source: &str) -> bool {
    attr.has_annotation
        && !attr.is_init_false
        && !attr.is_kw_only
        && !annotation_is_classvar(source, attr.annotation_span)
}

fn check_class(class: &ClassInfo, source: &str, path: &str, out: &mut Vec<Diagnostic>) {
    let mut seen_default = false;
    for attr in &class.attributes {
        if !is_positional_init_field(attr, source) {
            continue;
        }
        if attr.has_value {
            seen_default = true;
        } else if seen_default {
            out.push(make_diagnostic(class, attr, path));
        }
    }
}

fn make_diagnostic(class: &ClassInfo, attr: &AttributeInfo, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Dataclass field `{}` without a default cannot follow a field with a default \
             in `{}`",
            attr.name, class.name
        ),
        attr.name_span,
        path,
        Some(format!(
            "Give `{}` a default, or move it before all defaulted fields",
            attr.name
        )),
        Some(
            "A dataclass `__init__` cannot place a non-default parameter after a default one"
                .to_owned(),
        ),
    )
}
