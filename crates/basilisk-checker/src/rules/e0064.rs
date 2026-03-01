//! BSK-E0064: Invalid argument in a `NamedTuple` constructor call.
//!
//! When a `NamedTuple` is instantiated using keyword arguments, Basilisk
//! validates each argument against the field names and field types declared
//! in the `NamedTuple(...)` definition.
//!
//! Two kinds of violation are caught:
//!
//! 1. **Unknown field** — a keyword whose name is not among the declared fields.
//! 2. **Type mismatch** — a keyword whose literal value is incompatible with the
//!    declared field type (e.g. passing a `str` literal for an `int` field).
//!
//! ```python
//! X: Final = "x"
//! Y: Final = "y"
//! N = NamedTuple("N", [(X, int), (Y, int)])
//!
//! N(x=3, y=4)        # OK
//! N(a=1)             # E: unknown field `a`
//! N(x="", y="")      # E: field `x` expects `int` but got `str`
//! ```

use std::collections::HashMap;

use basilisk_resolver::{NamedTupleDefInfo, ResolvedModule, RhsKind, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0064",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0064",
};

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: None,
        note: Some(
            "PEP 544/NamedTuple: keyword arguments must match declared fields".to_owned(),
        ),
    }
}

/// Returns a human-readable literal-type name when `rhs` is incompatible
/// with the declared field type annotation text, or `None` when the pairing
/// is acceptable.
fn keyword_rhs_mismatch(annotation: &str, rhs: &RhsKind) -> Option<&'static str> {
    let base = annotation
        .split('[')
        .next()
        .unwrap_or(annotation)
        .trim()
        .to_ascii_lowercase();

    match (base.as_str(), rhs) {
        ("int" | "bool" | "float" | "bytes", RhsKind::StrLiteral) => Some("str"),
        ("int" | "str" | "float", RhsKind::BytesLiteral) => Some("bytes"),
        ("int" | "str" | "bool", RhsKind::FloatLiteral) => Some("float"),
        ("str" | "bytes", RhsKind::IntLiteral) => Some("int"),
        _ => None,
    }
}

/// Emits BSK-E0064 for `NamedTuple` call sites with unknown fields or type mismatches.
pub(crate) struct InvalidNamedTupleCall;

impl Rule for InvalidNamedTupleCall {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build map from NamedTuple LHS name → definition.
        let nt_map: HashMap<&str, &NamedTupleDefInfo> = module
            .namedtuple_defs
            .iter()
            .map(|nt| (nt.lhs_name.as_str(), nt))
            .collect();

        for call in &module.calls {
            let Some(nt) = nt_map.get(call.callee.as_str()) else {
                continue;
            };

            // Check positional argument type mismatches (only when types are known).
            if nt.has_types && !call.args.is_empty() {
                check_positional_types(nt, call, &module.path, diagnostics);
            }

            // Only check keyword calls for unknown fields / keyword type mismatches.
            if call.keywords.is_empty() {
                continue;
            }

            check_keyword_args(nt, call, &module.path, diagnostics);
        }
    }
}

/// Check positional argument types against `NamedTuple` field types.
fn check_positional_types(
    nt: &NamedTupleDefInfo,
    call: &basilisk_resolver::CallSite,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (idx, (arg_rhs, _arg_span)) in call.args.iter().enumerate() {
        let Some(field_type) = nt.field_types.get(idx) else {
            break;
        };
        if let Some(got_type) = keyword_rhs_mismatch(field_type, arg_rhs) {
            let field_name = nt.field_names.get(idx).map_or("?", String::as_str);
            diagnostics.push(make_diagnostic(
                format!(
                    "Argument {} to `{}()` has type `{got_type}` but \
                     field `{field_name}` expects `{field_type}`",
                    idx + 1,
                    nt.lhs_name,
                ),
                call.span,
                path,
            ));
        }
    }
}

/// Check keyword arguments for unknown field names and type mismatches.
fn check_keyword_args(
    nt: &NamedTupleDefInfo,
    call: &basilisk_resolver::CallSite,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Build a field-name set for existence checks.
    let field_name_set: std::collections::HashSet<&str> =
        nt.field_names.iter().map(String::as_str).collect();

    // Build a field-name → field-type map for type checks (only when types are known).
    let field_type_map: HashMap<&str, &str> = if nt.has_types {
        nt.field_names
            .iter()
            .zip(nt.field_types.iter())
            .map(|(name, typ)| (name.as_str(), typ.as_str()))
            .collect()
    } else {
        HashMap::new()
    };

    for (kw_name, kw_rhs) in &call.keywords {
        if !field_name_set.contains(kw_name.as_str()) {
            // Unknown field.
            diagnostics.push(make_diagnostic(
                format!(
                    "Argument `{kw_name}` is not a field of `{}`; \
                     valid fields are: {}",
                    nt.lhs_name,
                    nt.field_names
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                call.span,
                path,
            ));
        } else if let Some(declared_type) = field_type_map.get(kw_name.as_str()) {
            // Type mismatch check (only when types are known).
            if let Some(got_type) = keyword_rhs_mismatch(declared_type, kw_rhs) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Field `{kw_name}` of `{}` expects `{declared_type}` \
                         but received a `{got_type}` literal",
                        nt.lhs_name,
                    ),
                    call.span,
                    path,
                ));
            }
        }
    }
}
