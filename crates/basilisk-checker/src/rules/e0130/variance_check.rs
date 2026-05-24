//! Implements [BSK-E0130] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Variance assignment checking for BSK-E0130.
//!
//! Checks module-level and function-body assignments for variance
//! compatibility with inferred type parameter variances.

use std::collections::HashMap;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::{
    is_numeric_subtype, parse_subscript_annotation, split_top_level_commas,
};

use super::utils::span_for_line;
use super::variance::Variance;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0130",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0130",
};

/// Split at top-level commas (respecting brackets), returning owned trimmed strings.
pub(super) fn split_top_level_params(text: &str) -> Vec<String> {
    split_top_level_commas(text)
        .into_iter()
        .map(str::to_owned)
        .filter(|s| !s.trim().is_empty())
        .collect()
}

/// Check module-level assignments like `v: Class[A] = Class[B]()`.
pub(super) fn check_module_assignments(
    lines: &[&str],
    known: &HashMap<String, Vec<Variance>>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (idx, &line) in lines.iter().enumerate() {
        if line.starts_with(|c: char| c.is_whitespace()) {
            continue;
        }
        let trimmed = line.trim();
        // Skip comments — they are not executable code.
        if trimmed.starts_with('#') {
            continue;
        }
        let Some(colon) = trimmed.find(':') else {
            continue;
        };
        let Some(eq) = trimmed.find('=') else {
            continue;
        };
        if eq < colon || trimmed.get(eq + 1..eq + 2) == Some("=") {
            continue;
        }
        let ann = trimmed[colon + 1..eq].trim();
        let rhs = trimmed[eq + 1..].split('#').next().unwrap_or("").trim();

        let Some((lhs_cls, lhs_args)) = parse_subscript_annotation(ann) else {
            continue;
        };
        let Some((rhs_cls, rhs_args)) = extract_rhs_generic(rhs) else {
            continue;
        };
        if lhs_cls != rhs_cls {
            continue;
        }
        if let Some(vars) = known.get(lhs_cls) {
            emit_violations(
                &ViolationCtx {
                    class_name: lhs_cls,
                    lhs_args: &lhs_args,
                    rhs_args: &rhs_args,
                    variances: vars,
                    source,
                    path,
                    line_number: idx + 1,
                },
                diagnostics,
            );
        }
    }
}

/// Check assignments inside function bodies.
pub(super) fn check_fn_body_assignments(
    lines: &[&str],
    known: &HashMap<String, Vec<Variance>>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut param_types: HashMap<String, (String, Vec<String>)> = HashMap::new();
    let mut in_fn = false;
    let mut fn_indent = 0usize;

    for (idx, &line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();

        if trimmed.starts_with("def ") {
            in_fn = true;
            fn_indent = indent;
            param_types.clear();
            if let (Some(open), Some(close)) = (trimmed.find('('), trimmed.rfind(')')) {
                for p in split_top_level_params(&trimmed[open + 1..close]) {
                    let p = p.trim();
                    if let Some(c) = p.find(':') {
                        let name = p[..c].trim();
                        let ann = p[c + 1..].split('=').next().unwrap_or("").trim();
                        if let Some((cls, args)) = parse_subscript_annotation(ann) {
                            let _ = param_types.insert(name.to_owned(), (cls.to_owned(), args));
                        }
                    }
                }
            }
            continue;
        }

        if in_fn && indent <= fn_indent && !trimmed.is_empty() {
            in_fn = false;
            param_types.clear();
        }
        if !in_fn || param_types.is_empty() {
            continue;
        }

        let Some(colon) = trimmed.find(':') else {
            continue;
        };
        let Some(eq) = trimmed.find('=') else {
            continue;
        };
        if eq < colon || trimmed.get(eq + 1..eq + 2) == Some("=") {
            continue;
        }
        let ann = trimmed[colon + 1..eq].trim();
        let rhs = trimmed[eq + 1..].split('#').next().unwrap_or("").trim();

        if let Some((rhs_cls, rhs_args)) = param_types.get(rhs) {
            if let Some((lhs_cls, lhs_args)) = parse_subscript_annotation(ann) {
                if lhs_cls == rhs_cls {
                    if let Some(vars) = known.get(lhs_cls) {
                        emit_violations(
                            &ViolationCtx {
                                class_name: lhs_cls,
                                lhs_args: &lhs_args,
                                rhs_args,
                                variances: vars,
                                source,
                                path,
                                line_number: idx + 1,
                            },
                            diagnostics,
                        );
                    }
                }
            }
        }
    }
}

/// Extract generic type from RHS like `ClassName[Type](args)`.
fn extract_rhs_generic(rhs: &str) -> Option<(String, Vec<String>)> {
    let bracket = rhs.find('[')?;
    let class_name = rhs[..bracket].trim().to_owned();
    let after = &rhs[bracket + 1..];
    let mut depth = 0i32;
    let mut close = None;
    for (i, ch) in after.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' if depth == 0 => {
                close = Some(i);
                break;
            }
            ']' => depth -= 1,
            _ => {}
        }
    }
    let args: Vec<String> = split_top_level_commas(&after[..close?])
        .iter()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    if args.is_empty() {
        None
    } else {
        Some((class_name, args))
    }
}

/// Context for emitting variance violation diagnostics.
struct ViolationCtx<'a> {
    class_name: &'a str,
    lhs_args: &'a [String],
    rhs_args: &'a [String],
    variances: &'a [Variance],
    source: &'a str,
    path: &'a str,
    line_number: usize,
}

/// Emit diagnostics for variance violations between LHS and RHS type args.
fn emit_violations(ctx: &ViolationCtx<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for (idx, var) in ctx.variances.iter().enumerate() {
        let (Some(lhs), Some(rhs)) = (ctx.lhs_args.get(idx), ctx.rhs_args.get(idx)) else {
            continue;
        };
        if lhs == rhs {
            continue;
        }
        let ok = match var {
            Variance::Covariant => is_numeric_subtype(rhs, lhs),
            Variance::Contravariant => is_numeric_subtype(lhs, rhs),
            Variance::Invariant => false,
        };
        if ok {
            continue;
        }
        let label = match var {
            Variance::Covariant => "covariant",
            Variance::Contravariant => "contravariant",
            Variance::Invariant => "invariant",
        };
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Type `{}[{rhs}]` is not assignable to \
                 `{}[{lhs}]` (type parameter is {label})",
                ctx.class_name, ctx.class_name
            ),
            span_for_line(ctx.source, ctx.line_number),
            ctx.path,
            Some(format!(
                "{label} type parameter requires {}",
                match var {
                    Variance::Covariant => "subtype relationship (e.g. int → float)",
                    Variance::Contravariant => "supertype relationship (e.g. float → int)",
                    Variance::Invariant => "exact type match",
                }
            )),
            Some("PEP 695: variance is inferred from type parameter usage positions".to_owned()),
        ));
    }
}
