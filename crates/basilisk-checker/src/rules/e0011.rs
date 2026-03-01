//! BSK-E0011: Return type mismatch.
//!
//! Emitted when the inferred return type from function body expressions
//! is not assignable to the declared return type annotation.
//!
//! ```python
//! # BAD
//! def func() -> int:
//!     return "hello"  # E: inferred return type str is not assignable to int
//!
//! # GOOD  
//! def func() -> int:
//!     return 42  # OK: inferred return type int matches annotation
//! ```

use basilisk_resolver::{FunctionInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::{guards::is_stub_context, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0011",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0011",
};

/// Emits BSK-E0011 when inferred return types don't match the annotation.
pub(crate) struct ReturnTypeMismatch;

impl Rule for ReturnTypeMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for func in &module.functions {
            if is_stub_context(func, &module.classes) {
                continue;
            }
            
            // Skip if no return annotation or annotation is Any
            if !func.return_annotation.is_present() {
                continue;
            }
            
            // For now, we'll skip this check until we have proper return expression inference
            // This is a placeholder implementation that will be enhanced
            check_function_return_types(func, &module.path, diagnostics);
        }
    }
}

fn check_function_return_types(func: &FunctionInfo, path: &str, out: &mut Vec<Diagnostic>) {
    // Placeholder: This will be implemented once we have proper return expression inference
    // For now, we'll just check if there are any return statements with values
    let has_return_with_value = func.return_stmts.iter().any(|stmt| stmt.has_value);
    
    if has_return_with_value {
        // TODO: Implement proper return type inference once we have expression analysis
        // For now, we'll emit a diagnostic indicating this feature is not yet implemented
        out.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!("Return type inference for function `{}` not yet implemented", func.name),
            span: func.name_span,
            path: path.to_owned(),
            help: Some("Return type inference is part of the ongoing type inference sprint".to_owned()),
            note: Some("This check will be enhanced to properly infer return expression types".to_owned()),
        });
    }
}