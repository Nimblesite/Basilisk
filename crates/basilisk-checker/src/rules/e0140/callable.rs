//! Callable type parsing and compatibility checking for BSK-E0140.

use basilisk_resolver::Span;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::context::FuncSig;

// ---------------------------------------------------------------------------
// Callable type descriptor
// ---------------------------------------------------------------------------

/// Parsed representation of a `Callable[..., R]` annotation.
#[expect(dead_code, reason = "struct will be used for future type checking")]
pub(super) struct CallableTypeInfo {
    /// Explicit positional parameter types, or `None` for open-ended forms.
    pub(super) param_types: Option<Vec<String>>,
    /// Types from a `Concatenate[T1, T2, ..., P]` prefix.
    pub(super) concatenate_prefix: Vec<String>,
    /// Whether the callable accepts any argument list (`Callable[..., R]`).
    pub(super) is_open_ended: bool,
    /// Return type annotation text.
    #[expect(
        dead_code,
        reason = "return_type will be used for future type checking"
    )]
    pub(super) return_type: String,
}

// ---------------------------------------------------------------------------
// Callable annotation parsing
// ---------------------------------------------------------------------------

/// Parse a `Callable[..., R]` annotation string into a [`CallableTypeInfo`].
///
/// Returns `None` if the string is not a recognised `Callable` form.
pub(super) fn parse_callable_type(s: &str) -> Option<CallableTypeInfo> {
    if !s.starts_with("Callable[") {
        return None;
    }
    let inner = &s["Callable[".len()..s.len().checked_sub(1)?];
    let (first, ret) = split_top_comma(inner)?;
    let first = first.trim();
    let ret = ret.trim().to_owned();
    if first == "..." {
        return Some(CallableTypeInfo {
            param_types: None,
            concatenate_prefix: Vec::new(),
            is_open_ended: true,
            return_type: ret,
        });
    }
    if first.starts_with("Concatenate[") {
        let ci = &first["Concatenate[".len()..first.len().checked_sub(1)?];
        let parts = split_all_commas(ci);
        let mut prefix = Vec::new();
        let mut open = false;
        for p in &parts {
            let p = p.trim();
            if p == "..." {
                open = true;
            } else {
                prefix.push(p.to_owned());
            }
        }
        return Some(CallableTypeInfo {
            param_types: None,
            concatenate_prefix: prefix,
            is_open_ended: open,
            return_type: ret,
        });
    }
    if first.starts_with('[') && first.ends_with(']') {
        let li = &first[1..first.len() - 1];
        let types = if li.trim().is_empty() {
            Vec::new()
        } else {
            split_all_commas(li)
                .iter()
                .map(|s| s.trim().to_owned())
                .collect()
        };
        return Some(CallableTypeInfo {
            param_types: Some(types),
            concatenate_prefix: Vec::new(),
            is_open_ended: false,
            return_type: ret,
        });
    }
    None
}

// ---------------------------------------------------------------------------
// Callable compatibility checking
// ---------------------------------------------------------------------------

/// Check whether `func` is compatible with the `Callable` annotation described
/// by `ci`. Emits diagnostics for incompatibilities.
pub(super) fn check_callable_compat(
    ci: &CallableTypeInfo,
    func: &FuncSig,
    ann: &str,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
    code: &ErrorCode,
) {
    if !ci.concatenate_prefix.is_empty() {
        check_concatenate_prefix(ci, func, ann, path, diag, span, code);
        return;
    }
    if let Some(ptypes) = &ci.param_types {
        check_param_count_compat(ptypes, func, ann, path, diag, span, code);
    }
}

/// Check `Concatenate[T1, T2, ..., P]` prefix compatibility.
fn check_concatenate_prefix(
    ci: &CallableTypeInfo,
    func: &FuncSig,
    ann: &str,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
    code: &ErrorCode,
) {
    let req = ci.concatenate_prefix.len();
    let fpos = func.positional_params.len();
    if fpos == 0 && !func.kw_only_params.is_empty() {
        diag.push(Diagnostic {
            code: code.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{ann}`: Concatenate requires positional params",
                func.name
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
        return;
    }
    if fpos < req {
        diag.push(Diagnostic {
            code: code.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{ann}`: needs at least {req} positional param(s) but has {fpos}",
                func.name
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
        return;
    }
    for (idx, exp) in ci.concatenate_prefix.iter().enumerate() {
        if let Some(param) = func.positional_params.get(idx) {
            let act = &param.type_annotation;
            if !act.is_empty() && !types_compat(exp, act) {
                diag.push(Diagnostic {
                    code: code.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Function `{}` incompatible with `{ann}`: param {} type `{act}` vs required `{exp}`",
                        func.name,
                        idx + 1
                    ),
                    span,
                    path: path.to_owned(),
                    help: None,
                    note: None,
                });
            }
        }
    }
}

/// Check that the callable's parameter count is compatible with the function's.
fn check_param_count_compat(
    ptypes: &[String],
    func: &FuncSig,
    ann: &str,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
    code: &ErrorCode,
) {
    let exp = ptypes.len();
    let min = func
        .positional_params
        .iter()
        .filter(|p| !p.has_default)
        .count();
    let max = func.positional_params.len();
    if exp < min {
        diag.push(Diagnostic {
            code: code.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{ann}`: callable provides {exp} args but function requires {min}",
                func.name
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
    } else if exp > max && !func.has_varargs {
        diag.push(Diagnostic {
            code: code.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{ann}`: callable provides {exp} args but function accepts {max}",
                func.name
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
    }
}

// ---------------------------------------------------------------------------
// String splitting utilities
// ---------------------------------------------------------------------------

/// Split `s` at the first top-level comma, returning `(before, after)`.
pub(super) fn split_top_comma(s: &str) -> Option<(&str, &str)> {
    let mut d: usize = 0;
    for (i, c) in s.char_indices() {
        match c {
            '[' | '(' => d += 1,
            ']' | ')' => d = d.saturating_sub(1),
            ',' if d == 0 => return Some((&s[..i], &s[i + 1..])),
            _ => {}
        }
    }
    None
}

/// Split `s` at every top-level comma, returning all parts.
pub(super) fn split_all_commas(s: &str) -> Vec<&str> {
    let mut d: usize = 0;
    let mut parts = Vec::new();
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '[' | '(' => d += 1,
            ']' | ')' => d = d.saturating_sub(1),
            ',' if d == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

// ---------------------------------------------------------------------------
// Type compatibility
// ---------------------------------------------------------------------------

/// Returns `true` when `source` is assignable to `target` for the purposes of
/// callable/protocol parameter checking.
pub(super) fn types_compat(target: &str, source: &str) -> bool {
    if target == source {
        return true;
    }
    if target == "Any" || source == "Any" {
        return true;
    }
    if target.is_empty() || source.is_empty() {
        return true;
    }
    if target == "int" && source == "float" {
        return true;
    }
    if target == "float" && source == "int" {
        return true;
    }
    if target == "bool" && source == "int" {
        return true;
    }
    if target.contains(" | ") {
        return target.split(" | ").any(|m| m.trim() == source);
    }
    let builtins = [
        "int", "str", "float", "bool", "bytes", "None", "complex", "object",
    ];
    if builtins.contains(&target) && builtins.contains(&source) {
        return false;
    }
    true
}
