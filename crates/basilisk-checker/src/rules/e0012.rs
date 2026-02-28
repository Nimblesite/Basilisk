//! BSK-E0012: Argument type mismatch at a call site.
//!
//! When a function is called with a literal argument whose type is clearly
//! incompatible with the declared parameter annotation, Basilisk reports the
//! mismatch.  The check mirrors the literal-kind vs annotation comparison
//! used by BSK-E0014.
//!
//! ```python
//! def add(x: int, y: int) -> int:
//!     return x + y
//!
//! result: int = add("hello", "world")   # str literals for int params → E0012
//! ```

use basilisk_resolver::{FunctionInfo, ResolvedModule, RhsKind, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0012",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0012",
};

/// Emits BSK-E0012 for call sites where a literal argument is incompatible
/// with the declared parameter type.
pub(crate) struct ArgumentTypeMismatch;

impl Rule for ArgumentTypeMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build map from function name → &FunctionInfo (module-level functions only).
        let func_map: std::collections::HashMap<&str, &FunctionInfo> = module
            .functions
            .iter()
            .filter(|f| f.class_name.is_none())
            .map(|f| (f.name.as_str(), f))
            .collect();

        for call in &module.calls {
            let Some(func) = func_map.get(call.callee.as_str()) else {
                continue;
            };

            for (arg_idx, (rhs_kind, arg_span)) in call.args.iter().enumerate() {
                let Some(param) = func.parameters.get(arg_idx) else {
                    break;
                };

                let Some(ann_span) = param.annotation_span else {
                    continue;
                };
                let Some(ann_text) = module
                    .source
                    .get(ann_span.start as usize..ann_span.end as usize)
                else {
                    continue;
                };

                if let Some(description) = arg_rhs_mismatch(ann_text, rhs_kind) {
                    diagnostics.push(make_diagnostic(
                        &call.callee,
                        &param.name,
                        ann_text,
                        description,
                        *arg_span,
                        &module.path,
                    ));
                }
            }
        }
    }
}

/// Returns a human-readable description when `rhs` is incompatible with
/// the annotation text, or `None` when the pairing is acceptable.
fn arg_rhs_mismatch(annotation: &str, rhs: &RhsKind) -> Option<&'static str> {
    let base = annotation
        .split('[')
        .next()
        .unwrap_or(annotation)
        .trim()
        .to_ascii_lowercase();

    match (base.as_str(), rhs) {
        ("int" | "bool" | "float" | "bytes", RhsKind::StrLiteral) => Some("a `str` literal"),
        ("int" | "str" | "float", RhsKind::BytesLiteral) => Some("a `bytes` literal"),
        ("int" | "str" | "bool", RhsKind::FloatLiteral) => Some("a `float` literal"),
        ("str" | "bytes", RhsKind::IntLiteral) => Some("an `int` literal"),
        _ => None,
    }
}

fn make_diagnostic(
    callee: &str,
    param_name: &str,
    annotation: &str,
    rhs_description: &str,
    span: Span,
    path: &str,
) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Argument `{param_name}` of `{callee}` expects `{annotation}` but received \
             {rhs_description}"
        ),
        span,
        path: path.to_owned(),
        help: Some(format!(
            "Pass a value of type `{annotation}` for parameter `{param_name}`"
        )),
        note: Some(
            "Basilisk checks that literal arguments are compatible with declared parameter types"
                .to_owned(),
        ),
    }
}
