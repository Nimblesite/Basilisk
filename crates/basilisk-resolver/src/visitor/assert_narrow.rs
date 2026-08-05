//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE

use std::collections::{HashMap, HashSet};

use ruff_python_ast::Stmt;

use crate::scope::AssertTypeCallInfo;

use super::typeddict::{split_subscript, split_top_level_args};

pub(super) fn collect(_stmts: &[Stmt], _source: &str) -> Vec<AssertTypeCallInfo> {
    Vec::new()
}

/// Structurally match `pattern` against `actual`, binding any type variable in
/// `tvars` to the corresponding `actual` sub-expression.
pub(super) fn bind_type_vars(
    pattern: &str,
    actual: &str,
    tvars: &HashSet<String>,
    out: &mut HashMap<String, String>,
) {
    let pattern = pattern.trim();
    let actual = actual.trim();
    if tvars.contains(pattern) {
        let _ = out
            .entry(pattern.to_owned())
            .or_insert_with(|| actual.to_owned());
        return;
    }
    if let (Some((ph, pi)), Some((ah, ai))) = (split_subscript(pattern), split_subscript(actual)) {
        if ph == ah {
            let pargs = split_top_level_args(pi);
            let aargs = split_top_level_args(ai);
            if pargs.len() == aargs.len() {
                for (pa, aa) in pargs.iter().zip(aargs.iter()) {
                    bind_type_vars(pa, aa, tvars, out);
                }
            }
        }
    }
}

/// Replace whole-identifier tokens in `ty` with their bindings.
pub(super) fn substitute_type_vars(ty: &str, bindings: &HashMap<String, String>) -> String {
    if bindings.is_empty() {
        return ty.to_owned();
    }
    let mut result = String::with_capacity(ty.len());
    let mut ident = String::new();
    let flush = |ident: &mut String, result: &mut String| {
        if !ident.is_empty() {
            match bindings.get(ident.as_str()) {
                Some(sub) => result.push_str(sub),
                None => result.push_str(ident),
            }
            ident.clear();
        }
    };
    for ch in ty.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            ident.push(ch);
        } else {
            flush(&mut ident, &mut result);
            result.push(ch);
        }
    }
    flush(&mut ident, &mut result);
    result
}
