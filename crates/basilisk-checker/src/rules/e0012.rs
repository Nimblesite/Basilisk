//! Implements [BSK-E0012] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
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

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule, RhsKind, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0012",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0012",
};

/// Emits BSK-E0012 for call sites where a literal argument is incompatible
/// with the declared parameter type.
pub(crate) struct ArgumentTypeMismatch;

impl Rule for ArgumentTypeMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Group module-level functions by name → list of overloads/implementations.
        let mut func_groups: HashMap<&str, Vec<&FunctionInfo>> = HashMap::new();
        for func in &module.functions {
            if func.class_name.is_none() {
                func_groups
                    .entry(func.name.as_str())
                    .or_default()
                    .push(func);
            }
        }

        for call in &module.calls {
            // Only check calls to locally-defined functions for now.
            // Cross-module argument checking requires parsing imported function
            // signatures from `ExternalSymbol` — future work (Phase 4+).
            let Some(funcs) = func_groups.get(call.callee.as_str()) else {
                continue;
            };

            // Determine which function to check arguments against.
            let func_to_check = resolve_overload_for_call(funcs, call.args.len(), module);

            let Some(func) = func_to_check else {
                continue;
            };

            for (arg_idx, (rhs_kind, arg_span)) in call.args.iter().enumerate() {
                let Some(param) = func.parameters.get(arg_idx) else {
                    break;
                };

                let Some(ann_span) = param.annotation_span else {
                    continue;
                };
                let Some(ann_text) = slice_span(&module.source, ann_span) else {
                    continue;
                };

                let arg_source = slice_span(&module.source, *arg_span);

                if let Some(description) = arg_rhs_mismatch(ann_text, rhs_kind, arg_source) {
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

/// For an overloaded function, determine which function signature to check
/// arguments against.
///
/// If there is only one function (no overloads), returns it directly.
/// If there are overloads, filters by arity — if exactly one overload matches
/// the provided argument count, returns that overload. Otherwise returns the
/// implementation (non-overload) function.
fn resolve_overload_for_call<'a>(
    funcs: &[&'a FunctionInfo],
    arg_count: usize,
    module: &ResolvedModule,
) -> Option<&'a FunctionInfo> {
    if funcs.len() <= 1 {
        return funcs.first().copied();
    }

    // Separate overload stubs from the implementation.
    let overloads: Vec<&FunctionInfo> = funcs
        .iter()
        .filter(|f| is_overload_stub(f, module))
        .copied()
        .collect();

    if overloads.is_empty() {
        // No @overload decorators — just use the last function.
        return funcs.last().copied();
    }

    // Filter overloads by arity: keep those where the argument count is valid.
    let arity_matches: Vec<&FunctionInfo> = overloads
        .iter()
        .filter(|f| {
            // Skip functions with *args (they accept any number of positional args)
            if f.vararg.is_some() {
                return true;
            }
            let required = f.parameters.iter().filter(|p| !p.has_default).count();
            let total = f.parameters.len();
            arg_count >= required && arg_count <= total
        })
        .copied()
        .collect();

    match arity_matches.len() {
        0 => {
            // No overload matches arity — fall back to implementation.
            funcs.iter().find(|f| !is_overload_stub(f, module)).copied()
        }
        1 => {
            // Exactly one overload matches arity — check against it.
            arity_matches.first().copied()
        }
        _ => {
            // Multiple overloads match arity — fall back to implementation
            // to avoid false positives (full type-based overload resolution
            // would be needed to pick the right one).
            funcs.iter().find(|f| !is_overload_stub(f, module)).copied()
        }
    }
}

/// Returns `true` if the function has an `@overload` decorator and a stub body.
fn is_overload_stub(func: &FunctionInfo, _module: &ResolvedModule) -> bool {
    func.is_stub_body
        && func
            .decorators
            .iter()
            .any(|d| d == "overload" || d.ends_with(".overload"))
}

/// Returns a human-readable description when `rhs` is incompatible with
/// the annotation text, or `None` when the pairing is acceptable.
///
/// `arg_source` is the raw source text of the argument expression, used to
/// disambiguate `CallExpr` arguments (e.g. detecting `type(None)` vs other calls).
fn arg_rhs_mismatch(
    annotation: &str,
    rhs: &RhsKind,
    arg_source: Option<&str>,
) -> Option<&'static str> {
    let base = annotation
        .split('[')
        .next()
        .unwrap_or(annotation)
        .trim()
        .to_ascii_lowercase();

    // Check for TypeVarTuple unpack patterns in parameter annotations
    if annotation.contains("*tuple[Any, ...]") && matches!(rhs, RhsKind::CallExpr | RhsKind::Other)
    {
        return Some("a generic type that may be incompatible with TypeVarTuple unpacking");
    }

    match (base.as_str(), rhs) {
        ("int" | "bool" | "float" | "bytes", RhsKind::StrLiteral) => Some("a `str` literal"),
        ("int" | "str" | "float", RhsKind::BytesLiteral) => Some("a `bytes` literal"),
        ("int" | "str" | "bool", RhsKind::FloatLiteral) => Some("a `float` literal"),
        ("str" | "bytes", RhsKind::IntLiteral) => Some("an `int` literal"),
        // `None` literal passed where a class/type object is expected.
        // `type[X]` means a class object; passing `None` value is always wrong.
        ("type", RhsKind::NoneValue) => {
            Some("`None` (a value, not a class object — use `type(None)` or `NoneType`)")
        }
        // `type(None)` returns a class object (`NoneType`), not the value `None`.
        // A parameter annotated `None` expects the value `None`, not its type.
        ("none", RhsKind::TypeCall) => Some("`type(None)` (a class object, not the value `None`)"),
        // `type(None)` classified as a generic `CallExpr` by the resolver.
        // When the annotation is `None`, a `type(...)` call produces a class
        // object, which is incompatible with the `None` value type.
        ("none", RhsKind::CallExpr) if is_type_call(arg_source) => {
            Some("`type(None)` (a class object, not the value `None`)")
        }
        _ => None,
    }
}

/// Returns `true` when the argument source text is a `type(...)` call.
fn is_type_call(arg_source: Option<&str>) -> bool {
    let src = match arg_source {
        Some(s) => s.trim(),
        None => return false,
    };
    src.starts_with("type(") && src.ends_with(')')
}

fn make_diagnostic(
    callee: &str,
    param_name: &str,
    annotation: &str,
    rhs_description: &str,
    span: Span,
    path: &str,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Argument `{param_name}` of `{callee}` expects `{annotation}` but received \
             {rhs_description}"
        ),
        span,
        path,
        Some(format!(
            "Pass a value of type `{annotation}` for parameter `{param_name}`"
        )),
        Some(
            "Basilisk checks that literal arguments are compatible with declared parameter types"
                .to_owned(),
        ),
    )
}
