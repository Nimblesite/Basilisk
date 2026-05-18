//! BSK-E0136: Callable subtyping violations (covariance / contravariance).
//!
//! Callable types are covariant with respect to return types and contravariant
//! with respect to parameter types.  When a `Callable[[T], R]`-annotated
//! variable is assigned a value whose type is `Callable[[S], Q]`, the
//! assignment is only valid when:
//!
//! - `Q` is a subtype of `R`  (return type — covariant)
//! - `T` is a subtype of `S`  (parameter type — contravariant, i.e. the source
//!   must accept everything the target accepts, which means a broader type)
//!
//! ```python
//! def func(
//!     cb1: Callable[[float], int],
//!     cb3: Callable[[int], int],
//! ) -> None:
//!     f6: Callable[[float], float] = cb3  # E — int param is not supertype of float
//!     f8: Callable[[int], int] = cb2      # E — float return is not subtype of int
//! ```
//!
//! This rule specifically handles assignments inside function bodies where the
//! RHS is a parameter whose type is already known to be a `Callable`.

use ruff_python_ast::{self as ast, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, error_diagnostic_owned};
use crate::rules::shared::{ann_str, expr_name, is_numeric_subtype, split_top_level_commas};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0136",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0136",
};

/// Emits BSK-E0136 for callable-to-callable subtyping violations.
pub(crate) struct CallableSubtypingViolation;

impl Rule for CallableSubtypingViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };
        check_stmts(&parsed.ast.body, &module.path, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Parsed callable signature
// ---------------------------------------------------------------------------

/// The parsed components of a `Callable[[P1, P2, ...], R]` type string.
#[derive(Debug, Clone)]
struct CallableSig {
    param_types: Vec<String>,
    return_type: String,
}

/// Parse `Callable[[P1, ...], R]` into `(param_types, return_type)`.
/// Returns `None` for open-ended `Callable[..., R]` or parse failures.
fn parse_callable_sig(s: &str) -> Option<CallableSig> {
    let inner = s.strip_prefix("Callable[")?;
    let inner = inner.strip_suffix(']')?;

    // Split at the top-level comma between the param list and return type.
    let (params_part, ret_part) = split_top_level_comma(inner)?;
    let params_part = params_part.trim();
    let return_type = ret_part.trim().to_owned();

    // Open-ended callable: `Callable[..., R]` — no subtyping violations possible.
    if params_part == "..." {
        return None;
    }

    // Param list must be `[T1, T2, ...]`.
    let params_inner = params_part.strip_prefix('[')?.strip_suffix(']')?;
    let param_types = if params_inner.trim().is_empty() {
        Vec::new()
    } else {
        split_top_level_commas(params_inner)
            .into_iter()
            .map(|s| s.trim().to_owned())
            .collect()
    };

    Some(CallableSig {
        param_types,
        return_type,
    })
}

// ---------------------------------------------------------------------------
// Subtype / supertype relationships
// ---------------------------------------------------------------------------

/// Returns `true` when `candidate` is a subtype of `required`.
fn is_subtype(candidate: &str, required: &str) -> bool {
    if candidate == required || required == "object" || required == "Any" || candidate == "Any" {
        return true;
    }
    is_numeric_subtype(candidate, required)
}

/// Returns `true` when the return type of the *source* callable is compatible
/// with the return type of the *target* callable (covariant check).
///
/// The source return type must be a subtype of the target return type.
fn return_type_compat(source_ret: &str, target_ret: &str) -> bool {
    is_subtype(source_ret, target_ret)
}

/// Returns `true` when the parameter types of the *source* callable are
/// compatible with the parameter types of the *target* callable
/// (contravariant check).
///
/// The source parameter types must be supertypes of the corresponding target
/// parameter types.
fn param_types_compat(source_params: &[String], target_params: &[String]) -> bool {
    if source_params.len() != target_params.len() {
        // Arity mismatch — not a subtyping violation handled here.
        return true;
    }
    source_params
        .iter()
        .zip(target_params.iter())
        .all(|(src, tgt)| {
            // Contravariance: source param must be a supertype of target param,
            // i.e. `tgt` must be a subtype of `src`.
            is_subtype(tgt, src)
        })
}

// ---------------------------------------------------------------------------
// AST traversal
// ---------------------------------------------------------------------------

fn check_stmts(stmts: &[Stmt], path: &str, diag: &mut Vec<Diagnostic>) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                let param_callables = collect_callable_params(func);
                check_func_body(&func.body, &param_callables, path, diag);
            }
            Stmt::ClassDef(cls) => check_stmts(&cls.body, path, diag),
            _ => {}
        }
    }
}

/// Collect all parameters of `func` whose type annotation is a `Callable[...]`.
///
/// Returns a map from parameter name to the parsed `CallableSig`.
fn collect_callable_params(
    func: &ast::StmtFunctionDef,
) -> std::collections::HashMap<String, CallableSig> {
    let mut map = std::collections::HashMap::new();
    let params = &func.parameters;

    let all_params = params
        .posonlyargs
        .iter()
        .chain(params.args.iter())
        .chain(params.kwonlyargs.iter());

    for param in all_params {
        let Some(ann) = &param.parameter.annotation else {
            continue;
        };
        let ann_text = ann_str(ann);
        if let Some(sig) = parse_callable_sig(&ann_text) {
            let _ = map.insert(param.parameter.name.to_string(), sig);
        }
    }
    map
}

/// Check all annotated assignments inside a function body for callable
/// subtyping violations.
fn check_func_body(
    stmts: &[Stmt],
    param_callables: &std::collections::HashMap<String, CallableSig>,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        if let Stmt::AnnAssign(ann) = stmt {
            let Some(value) = &ann.value else {
                continue;
            };
            let ann_text = ann_str(&ann.annotation);
            if !ann_text.starts_with("Callable[") {
                continue;
            }
            let Some(target_sig) = parse_callable_sig(&ann_text) else {
                continue;
            };
            // The RHS must be a simple name (a parameter reference).
            let Some(rhs_name) = expr_name(value) else {
                continue;
            };
            let Some(source_sig) = param_callables.get(rhs_name) else {
                continue;
            };

            let span = Span::from(ann.range());

            // Check return type covariance.
            if !return_type_compat(&source_sig.return_type, &target_sig.return_type) {
                diag.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Callable subtyping violation: return type `{}` is not a subtype \
                         of `{}` (return types must be covariant)",
                        source_sig.return_type, target_sig.return_type,
                    ),
                    span,
                    path,
                    Some(format!(
                        "The source callable returns `{}` but the target expects a subtype \
                         of `{}`",
                        source_sig.return_type, target_sig.return_type,
                    )),
                    Some(
                        "Callable types are covariant with respect to their return types \
                         (PEP 484)"
                            .to_owned(),
                    ),
                ));
                // Report at most one violation per assignment.
                continue;
            }

            // Check parameter type contravariance.
            if !param_types_compat(&source_sig.param_types, &target_sig.param_types) {
                diag.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Callable subtyping violation: parameter type(s) of `{rhs_name}` \
                         are not supertypes of `{ann_text}` parameter types (parameters must be \
                         contravariant)",
                    ),
                    span,
                    path,
                    Some(
                        "The source callable must accept at least every argument the \
                         target callable accepts (contravariance)"
                            .to_owned(),
                    ),
                    Some(
                        "Callable types are contravariant with respect to their parameter \
                         types (PEP 484)"
                            .to_owned(),
                    ),
                ));
            }
        }
    }
}

/// Split `s` at the first top-level comma (respecting bracket nesting).
fn split_top_level_comma(s: &str) -> Option<(&str, &str)> {
    let mut depth: usize = 0;
    for (i, c) in s.char_indices() {
        match c {
            '[' | '(' => depth = depth.saturating_add(1),
            ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => return Some((&s[..i], &s[i + 1..])),
            _ => {}
        }
    }
    None
}
