//! BSK-E0088: Invalid tuple type syntax.
//!
//! Validates tuple type annotations according to PEP 646 rules:
//!
//! - `tuple[T, ...]` must have exactly one type before `...`
//! - `tuple[...]` is invalid (must specify a type)
//! - `tuple[T, ..., U]` is invalid (`...` can only appear at the end)
//! - `tuple[T, U, ...]` is invalid (can't have multiple fixed types before `...`)
//! - Invalid unpack patterns like `tuple[*tuple[str], ...]`
//!
//! ```python
//! t1: tuple[int, ...]        # OK
//! t2: tuple[int, int, ...]   # E — multiple fixed types before ...
//! t3: tuple[...]             # E — missing type before ...
//! t4: tuple[..., int]         # E — ... must be at the end
//! t5: tuple[int, ..., int]    # E — ... must be at the end
//! t6: tuple[*tuple[str], ...] # E — invalid unpack pattern
//! ```

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0088",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0088",
};

/// Emits BSK-E0088 for invalid tuple type syntax.
pub(crate) struct InvalidTupleTypeSyntax;

impl Rule for InvalidTupleTypeSyntax {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        
        // Check all variable annotations for tuple type syntax violations
        for var in &module.module_vars {
            if !var.has_annotation {
                continue;
            }
            
            let Some(ann_span) = var.annotation_span else {
                continue;
            };
            
            let Some(ann_text) = source.get(ann_span.start as usize..ann_span.end as usize) else {
                continue;
            };
            
            let ann_trimmed = ann_text.trim();
            if let Some(error_msg) = check_tuple_syntax(ann_trimmed) {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!("Invalid tuple type syntax: {}", error_msg),
                    span: ann_span,
                    path: module.path.to_owned(),
                    help: Some("Use valid tuple type syntax according to PEP 646".to_owned()),
                    note: Some(
                        "Tuple types must follow the pattern `tuple[T, ...]` with exactly one type before the ellipsis"
                            .to_owned(),
                    ),
                });
            }
        }
        
        // Also check function return type annotations
        for func in &module.functions {
            if let Some(ret_span) = func.return_annotation_span {
                let Some(ret_text) = source.get(ret_span.start as usize..ret_span.end as usize) else {
                    continue;
                };
                
                let ret_trimmed = ret_text.trim();
                if let Some(error_msg) = check_tuple_syntax(ret_trimmed) {
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!("Invalid tuple type syntax: {}", error_msg),
                        span: ret_span,
                        path: module.path.to_owned(),
                        help: Some("Use valid tuple type syntax according to PEP 646".to_owned()),
                        note: Some(
                            "Tuple types must follow the pattern `tuple[T, ...]` with exactly one type before the ellipsis"
                                .to_owned(),
                        ),
                    });
                }
            }
        }
    }
}

/// Returns `Some(error_message)` if the tuple type annotation has invalid syntax.
fn check_tuple_syntax(annotation: &str) -> Option<&'static str> {
    // Check if this is a tuple annotation
    if !annotation.starts_with("tuple[") || !annotation.ends_with(']') {
        return None;
    }
    
    let inner = &annotation["tuple[".len()..annotation.len() - 1].trim();
    
    // Check for empty tuple: tuple[()]
    if inner == "()" {
        return None; // Valid empty tuple syntax
    }
    
    // Check for variadic tuple: tuple[T, ...]
    if inner.ends_with(", ...") {
        let before_ellipsis = &inner[..inner.len() - 5].trim(); // Remove ", ..."
        
        // Check if there's a type before the ellipsis
        if before_ellipsis.is_empty() {
            return Some("tuple[...] is invalid — must specify a type before the ellipsis");
        }
        
        // Check if there are multiple types before the ellipsis
        if before_ellipsis.contains(',') {
            return Some("tuple[T, U, ...] is invalid — can only have one type before the ellipsis");
        }
        
        // Check for invalid patterns like tuple[..., T] or tuple[T, ..., U]
        if inner.contains("...,") && !inner.ends_with(", ...") {
            return Some("ellipsis (...) must appear at the end of the tuple type");
        }
        
        return None; // Valid variadic tuple syntax
    }
    
    // Check for invalid ellipsis usage (not at the end)
    if inner.contains("...") {
        return Some("ellipsis (...) must appear at the end of the tuple type");
    }
    
    // Check for invalid unpack patterns
    if inner.contains("*tuple[") {
        // Patterns like tuple[*tuple[str], ...] are invalid
        if inner.contains(", ...") {
            return Some("invalid unpack pattern — cannot combine starred tuple unpack with ellipsis");
        }
    }
    
    None
}